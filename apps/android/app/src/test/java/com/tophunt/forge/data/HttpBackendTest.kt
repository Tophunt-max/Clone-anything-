package com.tophunt.forge.data

import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import java.util.concurrent.atomic.AtomicInteger

/** The app's HTTP client against a scripted server: tokens, refresh rotation, errors, offline. */
class HttpBackendTest {
    private lateinit var server: MockWebServer
    private val tokens = InMemoryTokenStore()
    private val settings = InMemorySettingsStore()
    private lateinit var backend: HttpBackend

    private fun json(code: Int, body: String) = MockResponse().setResponseCode(code).setHeader("content-type", "application/json").setBody(body)
    private val user = """{"id":"u1","email":"a@b.co","display_name":"Ada","created_at":"x"}"""
    private fun session(a: String, r: String) = """{"user":$user,"session_id":"s1","access_expires_at":"x","access_token":"$a","refresh_token":"$r"}"""
    private val unauth = """{"type":"x","title":"Sign in required","status":401,"code":"unauthenticated","detail":"Sign in again.","request_id":"rid-1"}"""

    @Before
    fun start() {
        server = MockWebServer()
        server.start()
        backend = HttpBackend(tokens, settings, allowLocalHttp = true, deviceName = "Test phone")
    }

    @After
    fun stop() = server.shutdown()

    private fun origin() = ServerUrl.parse("http://localhost:${server.port}", allowLocalHttp = true).getOrThrow()

    private fun route(handler: (RecordedRequest) -> MockResponse) {
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest) = handler(request)
        }
    }

    @Test
    fun connectChecksTheServerThenSavesIt() = runBlocking {
        route { if (it.path == "/api/v1/meta") json(200, """{"name":"Tophunt Forge","version":"0.1.0","api_version":"v1","features":[]}""") else json(404, "{}") }
        val meta = backend.connect(origin())
        assertEquals("v1", meta.apiVersion)
        assertEquals(origin().origin, settings.server())
    }

    @Test
    fun aServerThatIsNotOursIsNotSaved() = runBlocking {
        route { MockResponse().setResponseCode(200).setBody("<html>") }
        try {
            backend.connect(origin())
            fail("should not connect")
        } catch (_: Exception) {}
        assertNull(settings.server())
        assertNull(backend.server)
    }

    @Test
    fun loginKeepsTheTokensAndSendsBearer() = runBlocking {
        route {
            when (it.path) {
                "/api/v1/meta" -> json(200, """{"name":"n","version":"v","api_version":"v1"}""")
                "/api/v1/auth/login" -> json(200, session("a1", "r1"))
                "/api/v1/me" -> if (it.getHeader("Authorization") == "Bearer a1") json(200, user) else json(401, unauth)
                else -> json(404, "{}")
            }
        }
        backend.connect(origin())
        assertEquals("Ada", backend.login("a@b.co", "pw").displayName)
        assertEquals(Tokens("a1", "r1"), tokens.read())
        assertEquals("u1", backend.me().id)
        val login = (1..server.requestCount).map { server.takeRequest() }.first { it.path == "/api/v1/auth/login" }
        assertTrue(login.body.readUtf8().contains("\"client\":\"android\""))
        assertTrue(login.getHeader("X-Request-Id")!!.length == 36)
    }

    @Test
    fun a401RefreshesOnceForConcurrentCallsAndRetries() = runBlocking {
        val refreshes = AtomicInteger()
        route {
            when (it.path) {
                "/api/v1/meta" -> json(200, """{"name":"n","version":"v","api_version":"v1"}""")
                "/api/v1/auth/refresh" -> {
                    refreshes.incrementAndGet()
                    if (it.body.readUtf8().contains("\"r1\"")) json(200, session("a2", "r2")) else json(401, unauth)
                }
                "/api/v1/me" -> if (it.getHeader("Authorization") == "Bearer a2") json(200, user) else json(401, unauth)
                else -> json(404, "{}")
            }
        }
        backend.connect(origin())
        tokens.write(Tokens("old", "r1"))
        val results = (1..4).map { async(kotlinx.coroutines.Dispatchers.IO) { backend.me() } }.awaitAll()
        assertTrue(results.all { it.id == "u1" })
        assertEquals(1, refreshes.get())
        assertEquals(Tokens("a2", "r2"), tokens.read())
    }

    @Test
    fun aRefusedRefreshEndsTheSession() = runBlocking {
        route {
            when (it.path) {
                "/api/v1/meta" -> json(200, """{"name":"n","version":"v","api_version":"v1"}""")
                else -> json(401, unauth)
            }
        }
        backend.connect(origin())
        tokens.write(Tokens("old", "dead"))
        val ended = async { withTimeout(5_000) { backend.sessionEnded.first() } }
        try {
            backend.me()
            fail("should be refused")
        } catch (e: ApiException) {
            assertEquals(401, e.status)
            assertEquals("rid-1", e.requestId)
        }
        ended.await()
        assertNull(tokens.read())
    }

    @Test
    fun problemsAndOfflineAreTypedErrors() = runBlocking {
        route {
            when (it.path) {
                "/api/v1/meta" -> json(200, """{"name":"n","version":"v","api_version":"v1"}""")
                else -> json(401, """{"type":"x","title":"Not signed in","status":401,"code":"invalid_credentials","detail":"The email or password is wrong.","request_id":"rid-2"}""")
            }
        }
        backend.connect(origin())
        try {
            backend.login("a@b.co", "nope")
            fail()
        } catch (e: ApiException) {
            assertEquals("invalid_credentials", e.code)
            assertEquals("The email or password is wrong.", e.message)
            assertEquals("rid-2", e.requestId)
        }
        server.shutdown()
        try {
            backend.me()
            fail()
        } catch (e: OfflineException) {
            // expected
        }
    }

    @Test
    fun logoutAlwaysClearsTheLocalSession() = runBlocking {
        route { if (it.path == "/api/v1/meta") json(200, """{"name":"n","version":"v","api_version":"v1"}""") else json(500, "{}") }
        backend.connect(origin())
        tokens.write(Tokens("a", "r"))
        backend.logout()
        assertNull(tokens.read())
    }
}
