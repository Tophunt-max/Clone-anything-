package com.tophunt.forge.web

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.net.URI

/**
 * Security rules of the project WebView (threat model T7–T9), kept free of Android types so they
 * are unit-tested on the JVM.
 */
object WebPolicy {
    enum class Nav { STAY, OPEN_EXTERNALLY, BLOCK }

    fun originOf(url: String): String? = runCatching {
        val u = URI(url)
        val scheme = u.scheme?.lowercase() ?: return null
        val host = u.host?.lowercase() ?: return null
        if (scheme != "https" && scheme != "http") return null
        val default = if (scheme == "https") 443 else 80
        "$scheme://$host" + if (u.port == -1 || u.port == default) "" else ":${u.port}"
    }.getOrNull()

    /** Only the configured origin stays inside; other HTTPS pages open in the browser; anything else is blocked. */
    fun navigation(url: String, trustedOrigin: String): Nav {
        val origin = originOf(url)
        return when {
            origin == trustedOrigin -> Nav.STAY
            origin != null && origin.startsWith("https://") -> Nav.OPEN_EXTERNALLY
            else -> Nav.BLOCK
        }
    }

    /** Whether a bridge message may be handled: it must come from the trusted origin's main frame. */
    fun acceptsMessage(sourceOrigin: String?, isMainFrame: Boolean, trustedOrigin: String): Boolean = isMainFrame && sourceOrigin == trustedOrigin

    /** The page to open for a project: always a path on the trusted origin. */
    fun projectPath(projectId: String): String {
        require(projectId.matches(Regex("[0-9a-fA-F-]{1,64}"))) { "not a project id" }
        return "/p/$projectId"
    }
}

/** Requests the trusted web app may send to the native app. Nothing else exists. */
sealed interface BridgeRequest {
    val id: String

    data class AppInfo(override val id: String) : BridgeRequest
    data class Share(override val id: String, val text: String) : BridgeRequest
    data class OpenExternal(override val id: String, val url: String) : BridgeRequest
    data class Haptic(override val id: String) : BridgeRequest
}

@Serializable
private data class Envelope(val id: String, val method: String, val params: JsonObject = JsonObject(emptyMap()))

@Serializable
data class BridgeReply(val id: String, val ok: Boolean, val result: Map<String, String>? = null, val error: String? = null)

object Bridge {
    const val OBJECT_NAME = "forgeNative"
    const val MAX_MESSAGE = 8 * 1024
    const val MAX_SHARE = 2_000
    private val strict = Json { ignoreUnknownKeys = false; isLenient = false }
    private val ID = Regex("[A-Za-z0-9_-]{1,64}")

    /** Parses and validates one message. Any problem is a refusal with a reason, never a crash. */
    fun parse(raw: String?): Result<BridgeRequest> {
        if (raw == null || raw.length > MAX_MESSAGE) return refuse("too large or empty")
        val env = try {
            strict.decodeFromString<Envelope>(raw)
        } catch (e: SerializationException) {
            return refuse("not a valid message")
        } catch (e: IllegalArgumentException) {
            return refuse("not a valid message")
        }
        if (!ID.matches(env.id)) return refuse("bad id")
        fun param(name: String): String? = env.params[name]?.let { runCatching { it.jsonPrimitive.content }.getOrNull() }
        fun only(vararg names: String) = env.params.keys.all { it in names }
        return when (env.method) {
            "getAppInfo" -> if (env.params.isEmpty()) Result.success(BridgeRequest.AppInfo(env.id)) else refuse("unexpected params")
            "haptic" -> if (env.params.isEmpty()) Result.success(BridgeRequest.Haptic(env.id)) else refuse("unexpected params")
            "share" -> {
                val text = param("text")
                when {
                    !only("text") -> refuse("unexpected params")
                    text.isNullOrBlank() || text.length > MAX_SHARE -> refuse("text must be 1–$MAX_SHARE characters")
                    else -> Result.success(BridgeRequest.Share(env.id, text))
                }
            }
            "openExternal" -> {
                val url = param("url")
                when {
                    !only("url") -> refuse("unexpected params")
                    url == null || url.length > 2048 || WebPolicy.originOf(url)?.startsWith("https://") != true -> refuse("only https addresses")
                    else -> Result.success(BridgeRequest.OpenExternal(env.id, url))
                }
            }
            else -> refuse("unknown method")
        }
    }

    fun reply(r: BridgeReply): String = Json.encodeToString(BridgeReply.serializer(), r)

    private fun refuse(why: String) = Result.failure<BridgeRequest>(IllegalArgumentException(why))
}
