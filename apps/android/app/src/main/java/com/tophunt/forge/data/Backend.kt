package com.tophunt.forge.data

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.Json
import okhttp3.Authenticator
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.Route
import retrofit2.HttpException
import retrofit2.Retrofit
import retrofit2.converter.kotlinx.serialization.asConverterFactory
import retrofit2.http.Body
import retrofit2.http.GET
import retrofit2.http.POST
import retrofit2.http.Path
import retrofit2.http.Query
import java.io.IOException
import java.util.UUID
import java.util.concurrent.TimeUnit

/** What the native screens need from the server. Screens depend on this, so tests can fake it. */
interface Backend {
    val server: ServerUrl?
    /** Emits when the session ended (refresh refused): the app returns to sign-in. */
    val sessionEnded: SharedFlow<Unit>
    suspend fun connect(server: ServerUrl): Meta
    suspend fun meta(): Meta
    suspend fun hasSession(): Boolean
    suspend fun me(): User
    suspend fun login(email: String, password: String): User
    suspend fun register(name: String, email: String, password: String): User
    suspend fun logout()
    suspend fun workspaces(): List<Workspace>
    suspend fun projects(workspace: String): List<Project>
    /** A single-use code that signs the WebView in (60 s). */
    suspend fun loginCode(): String
    suspend fun ready(): Boolean
}

interface ForgeService {
    @GET("api/v1/meta") suspend fun meta(): Meta
    @GET("readyz") suspend fun ready(): retrofit2.Response<Unit>
    @POST("api/v1/auth/login") suspend fun login(@Body b: LoginBody): SessionStart
    @POST("api/v1/auth/register") suspend fun register(@Body b: RegisterBody): SessionStart
    @POST("api/v1/auth/logout") suspend fun logout(): retrofit2.Response<Unit>
    @POST("api/v1/auth/login-code") suspend fun loginCode(@Body empty: Map<String, String> = emptyMap()): LoginCode
    @GET("api/v1/me") suspend fun me(): User
    @GET("api/v1/workspaces") suspend fun workspaces(@Query("limit") limit: Int = 200): Page<Workspace>
    @GET("api/v1/workspaces/{ws}/projects") suspend fun projects(@Path("ws") ws: String, @Query("limit") limit: Int = 200): Page<Project>
}

// encodeDefaults: `client = "android"` must be sent, or the server answers as to a browser (cookies, no tokens).
val WireJson = Json { ignoreUnknownKeys = true; explicitNulls = false; encodeDefaults = true }

/** Turns transport and HTTP failures into [ApiException] / [OfflineException]. */
suspend fun <T> apiCall(block: suspend () -> T): T = try {
    block()
} catch (e: HttpException) {
    throw problemOf(e.code(), e.response()?.errorBody()?.string(), e.response()?.headers()?.get("x-request-id"))
} catch (e: IOException) {
    throw OfflineException(e)
}

fun problemOf(status: Int, body: String?, requestId: String?): ApiException {
    val p = body?.let { runCatching { WireJson.decodeFromString<Problem>(it) }.getOrNull() }
    return if (p != null) {
        ApiException(status, p.code, p.detail.ifBlank { p.title }, p.requestId ?: requestId, p.field)
    } else {
        ApiException(status, "http_error", "The server answered with an unexpected error ($status).", requestId)
    }
}

/**
 * The real backend: Retrofit + OkHttp with Bearer tokens. A 401 triggers one refresh (rotating
 * the stored refresh token), then one retry; a refused refresh clears the tokens and emits
 * [sessionEnded].
 */
class HttpBackend(
    private val tokens: TokenStore,
    private val settings: SettingsStore,
    private val allowLocalHttp: Boolean,
    private val deviceName: String,
    private val clientBuilder: () -> OkHttpClient.Builder = { OkHttpClient.Builder() },
) : Backend {
    private val ended = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    override val sessionEnded: SharedFlow<Unit> = ended
    override var server: ServerUrl? = null
        private set
    private var service: ForgeService? = null
    private var plainClient: OkHttpClient? = null
    private val refreshLock = Mutex()

    suspend fun restore() {
        ServerUrl.restore(settings.server(), allowLocalHttp)?.let { use(it) }
    }

    private fun use(url: ServerUrl) {
        server = url
        val base = clientBuilder()
            .connectTimeout(10, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .followRedirects(false)
            .addInterceptor(Interceptor { chain -> chain.proceed(chain.request().newBuilder().header("X-Request-Id", UUID.randomUUID().toString()).header("Accept", "application/json").build()) })
            .build()
        plainClient = base
        val authed = base.newBuilder()
            .addInterceptor(Interceptor { chain ->
                val t = kotlinx.coroutines.runBlocking { tokens.read() }
                val req = chain.request()
                chain.proceed(if (t != null && req.header("Authorization") == null) req.newBuilder().header("Authorization", "Bearer ${t.access}").build() else req)
            })
            .authenticator(RefreshAuthenticator())
            .build()
        service = Retrofit.Builder().baseUrl(url.apiBase).client(authed).addConverterFactory(WireJson.asConverterFactory("application/json".toMediaType())).build().create(ForgeService::class.java)
    }

    private fun svc(): ForgeService = service ?: throw ApiException(0, "no_server", "Choose a server first.", null)

    private inner class RefreshAuthenticator : Authenticator {
        override fun authenticate(route: Route?, response: Response): Request? {
            // Only one retry per request, and never for the auth calls themselves.
            if (response.priorResponse != null || response.request.url.encodedPath.startsWith("/api/v1/auth/")) return null
            val sentWith = response.request.header("Authorization")?.removePrefix("Bearer ")
            val fresh = kotlinx.coroutines.runBlocking { refreshAfter(sentWith) } ?: return null
            return response.request.newBuilder().header("Authorization", "Bearer ${fresh.access}").build()
        }
    }

    /** Refreshes unless another request already did since [sentWith] was used. */
    private suspend fun refreshAfter(sentWith: String?): Tokens? = refreshLock.withLock {
        val current = tokens.read() ?: return null
        if (current.access != sentWith) return current
        val url = server ?: return null
        val client = plainClient ?: return null
        val body = WireJson.encodeToString(RefreshBody.serializer(), RefreshBody(current.refresh))
        val req = Request.Builder().url("${url.origin}/api/v1/auth/refresh").post(okhttp3.RequestBody.create("application/json".toMediaType(), body)).build()
        val result = runCatching { client.newCall(req).execute() }.getOrNull() ?: return null // offline: keep the session
        result.use { r ->
            if (!r.isSuccessful) {
                if (r.code == 401) {
                    tokens.clear()
                    ended.tryEmit(Unit)
                }
                return null
            }
            val s = runCatching { WireJson.decodeFromString<SessionStart>(r.body?.string().orEmpty()) }.getOrNull() ?: return null
            val next = Tokens(s.accessToken ?: return null, s.refreshToken ?: return null)
            tokens.write(next)
            return next
        }
    }

    private suspend fun keep(s: SessionStart): User {
        val access = s.accessToken
        val refresh = s.refreshToken
        if (access == null || refresh == null) throw ApiException(0, "bad_response", "The server didn't start an app session.", null)
        tokens.write(Tokens(access, refresh))
        return s.user
    }

    override suspend fun connect(server: ServerUrl): Meta {
        val previous = this.server
        use(server)
        return try {
            val meta = apiCall { svc().meta() }
            if (previous != server) tokens.clear()
            settings.setServer(server.origin)
            meta
        } catch (e: Exception) {
            if (previous != null) use(previous) else { this.server = null; service = null }
            throw e
        }
    }

    override suspend fun meta() = apiCall { svc().meta() }
    override suspend fun hasSession() = server != null && tokens.read() != null
    override suspend fun me() = apiCall { svc().me() }
    override suspend fun login(email: String, password: String) = keep(apiCall { svc().login(LoginBody(email.trim(), password, deviceName = deviceName)) })
    override suspend fun register(name: String, email: String, password: String) = keep(apiCall { svc().register(RegisterBody(email.trim(), password, name.trim(), deviceName = deviceName)) })
    override suspend fun logout() {
        try {
            apiCall { svc().logout() }
        } catch (_: Exception) {
            // Signing out locally always works; the server session expires on its own.
        } finally {
            tokens.clear()
        }
    }
    override suspend fun workspaces() = apiCall { svc().workspaces().items }
    override suspend fun projects(workspace: String) = apiCall { svc().projects(workspace).items }
    override suspend fun loginCode() = apiCall { svc().loginCode().code }
    override suspend fun ready() = try { apiCall { svc().ready() }.isSuccessful } catch (_: Exception) { false }
}
