package com.tophunt.forge.data

import java.net.URI

/**
 * The server the app talks to, reduced to its origin (`https://host[:port]`). The web app and the
 * API share this origin; the WebView is only ever trusted for it.
 */
@JvmInline
value class ServerUrl private constructor(val origin: String) {
    val apiBase: String get() = "$origin/"

    companion object {
        private val LOCAL_HOSTS = setOf("localhost", "127.0.0.1", "10.0.2.2")

        /**
         * Parses what a person typed. HTTPS is required; plain HTTP only for local development
         * hosts and only when [allowLocalHttp] (debug builds). Paths, queries, fragments and
         * credentials are refused rather than silently dropped.
         */
        fun parse(input: String, allowLocalHttp: Boolean): Result<ServerUrl> {
            val text = input.trim().let { if ("://" in it) it else "https://$it" }
            val uri = runCatching { URI(text) }.getOrNull() ?: return fail("That isn't a web address.")
            val scheme = uri.scheme?.lowercase()
            val host = uri.host?.lowercase() ?: return fail("Enter the server's address, like forge.example.com.")
            if (uri.rawUserInfo != null) return fail("Leave out the user name and password.")
            if (!uri.rawPath.isNullOrEmpty() && uri.rawPath != "/") return fail("Enter only the server address, without a path.")
            if (uri.rawQuery != null || uri.rawFragment != null) return fail("Enter only the server address.")
            when (scheme) {
                "https" -> {}
                "http" -> if (!(allowLocalHttp && host in LOCAL_HOSTS)) return fail("The server must use HTTPS.")
                else -> return fail("The server must use HTTPS.")
            }
            val defaultPort = if (scheme == "https") 443 else 80
            val port = if (uri.port == -1 || uri.port == defaultPort) "" else ":${uri.port}"
            if (uri.port !in -1..65535) return fail("That port isn't valid.")
            return Result.success(ServerUrl("$scheme://$host$port"))
        }

        /** For values the app stored itself; still checked, never trusted blindly. */
        fun restore(stored: String?, allowLocalHttp: Boolean): ServerUrl? = stored?.let { parse(it, allowLocalHttp).getOrNull() }

        private fun fail(message: String) = Result.failure<ServerUrl>(IllegalArgumentException(message))
    }
}
