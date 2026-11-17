package com.tophunt.forge.data

import kotlinx.coroutines.runBlocking
import okhttp3.FormBody
import okhttp3.OkHttpClient
import okhttp3.Request
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

/**
 * The app's real HTTP client against a running forge-api. Skipped unless FORGE_LIVE_API_URL is
 * set (e.g. http://127.0.0.1:18091), so it runs wherever the API runs: locally and in CI.
 */
class LiveApiTest {
    private val url = System.getenv("FORGE_LIVE_API_URL")

    @Test
    fun registerListAndHandOverToTheWebView() = runBlocking {
        assumeTrue("set FORGE_LIVE_API_URL to run", url != null)
        val tokens = InMemoryTokenStore()
        val backend = HttpBackend(tokens, InMemorySettingsStore(), allowLocalHttp = true, deviceName = "JVM test")
        val server = ServerUrl.parse(url!!, allowLocalHttp = true).getOrThrow()
        assertEquals("v1", backend.connect(server).apiVersion)
        assertTrue(backend.ready())

        val email = "android-${System.nanoTime()}@example.com"
        assertEquals("Droid", backend.register("Droid", email, "a long enough passphrase").displayName)
        assertTrue("an app session holds tokens", tokens.read() != null)
        val ws = backend.workspaces()
        assertEquals("owner", ws.single().role)
        assertEquals(emptyList<Project>(), backend.projects(ws.single().id))

        // The WebView hand-over, the way the WebView does it: a form POST with the code.
        val code = backend.loginCode()
        val resp = OkHttpClient.Builder().followRedirects(false).build().newCall(
            Request.Builder().url("${server.origin}/api/v1/auth/exchange").header("Sec-Fetch-Site", "none")
                .post(FormBody.Builder().add("code", code).add("next", "/projects").build()).build(),
        ).execute()
        resp.use {
            assertEquals(303, it.code)
            assertEquals("/projects", it.header("Location"))
            val cookies = it.headers("Set-Cookie")
            assertTrue(cookies.any { c -> c.startsWith("forge_rt=") && "HttpOnly" in c })
        }

        backend.logout()
        assertEquals(null, tokens.read())
        try {
            backend.login(email, "wrong password!!")
            error("should fail")
        } catch (e: ApiException) {
            assertEquals("invalid_credentials", e.code)
            assertTrue(e.requestId!!.length == 36)
        }
        assertEquals("Droid", backend.login(email, "a long enough passphrase").displayName)
    }
}
