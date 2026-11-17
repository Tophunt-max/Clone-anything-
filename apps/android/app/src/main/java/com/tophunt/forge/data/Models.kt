package com.tophunt.forge.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Wire types of the platform API (docs/platform/05-api-spec.md). Unknown fields are ignored so
// a newer server doesn't break an older app.

@Serializable
data class Meta(val name: String, val version: String, @SerialName("api_version") val apiVersion: String, val features: List<String> = emptyList())

@Serializable
data class Problem(val code: String, val title: String = "", val status: Int = 0, val detail: String = "", val field: String? = null, @SerialName("request_id") val requestId: String? = null)

@Serializable
data class User(val id: String, val email: String, @SerialName("display_name") val displayName: String)

@Serializable
data class SessionStart(
    val user: User,
    @SerialName("session_id") val sessionId: String,
    @SerialName("access_token") val accessToken: String? = null,
    @SerialName("refresh_token") val refreshToken: String? = null,
)

@Serializable
data class LoginBody(val email: String, val password: String, val client: String = "android", @SerialName("device_name") val deviceName: String? = null)

@Serializable
data class RegisterBody(
    val email: String,
    val password: String,
    @SerialName("display_name") val displayName: String,
    val client: String = "android",
    @SerialName("device_name") val deviceName: String? = null,
)

@Serializable
data class RefreshBody(@SerialName("refresh_token") val refreshToken: String)

@Serializable
data class Page<T>(val items: List<T>, @SerialName("next_cursor") val nextCursor: String? = null)

@Serializable
data class Workspace(val id: String, val name: String, val slug: String, val role: String)

@Serializable
data class Project(
    val id: String,
    @SerialName("workspace_id") val workspaceId: String,
    val name: String,
    val kind: String,
    @SerialName("updated_at") val updatedAt: String,
    @SerialName("archived_at") val archivedAt: String? = null,
)

@Serializable
data class LoginCode(val code: String, @SerialName("expires_in_secs") val expiresInSecs: Long)

/** The server refused: decide on [code], show [message], quote [requestId]. */
class ApiException(val status: Int, val code: String, override val message: String, val requestId: String?, val field: String? = null) : Exception(message)

/** The server could not be reached at all. */
class OfflineException(cause: Throwable) : Exception("Can't reach the server. Check your connection and the server address.", cause)
