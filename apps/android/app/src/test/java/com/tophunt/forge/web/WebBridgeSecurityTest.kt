package com.tophunt.forge.web

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/** WebView bridge and navigation security (threat model T7, T8). */
class WebBridgeSecurityTest {
    private val trusted = "https://forge.example.com"

    @Test
    fun onlyTheTrustedOriginStaysInsideTheWebView() {
        assertEquals(WebPolicy.Nav.STAY, WebPolicy.navigation("https://forge.example.com/p/1", trusted))
        assertEquals(WebPolicy.Nav.STAY, WebPolicy.navigation("https://FORGE.example.com:443/x", trusted))
        assertEquals(WebPolicy.Nav.OPEN_EXTERNALLY, WebPolicy.navigation("https://docs.example.org/", trusted))
        assertEquals(WebPolicy.Nav.OPEN_EXTERNALLY, WebPolicy.navigation("https://forge.example.com.evil.io/", trusted))
        assertEquals(WebPolicy.Nav.OPEN_EXTERNALLY, WebPolicy.navigation("https://forge.example.com:8443/", trusted))
        for (u in listOf("http://forge.example.com/", "javascript:alert(1)", "file:///sdcard/x", "intent://x#Intent;end", "data:text/html,hi", "content://x", "about:blank")) {
            assertEquals(u, WebPolicy.Nav.BLOCK, WebPolicy.navigation(u, trusted))
        }
    }

    @Test
    fun bridgeMessagesAreAcceptedOnlyFromTheTrustedMainFrame() {
        assertTrue(WebPolicy.acceptsMessage(trusted, isMainFrame = true, trustedOrigin = trusted))
        assertFalse(WebPolicy.acceptsMessage(trusted, isMainFrame = false, trustedOrigin = trusted))
        assertFalse(WebPolicy.acceptsMessage("https://evil.example", isMainFrame = true, trustedOrigin = trusted))
        assertFalse(WebPolicy.acceptsMessage(null, isMainFrame = true, trustedOrigin = trusted))
        assertFalse(WebPolicy.acceptsMessage("http://forge.example.com", isMainFrame = true, trustedOrigin = trusted))
    }

    @Test
    fun validMessagesParseIntoTheFixedSetOfRequests() {
        assertEquals(BridgeRequest.AppInfo("a1"), Bridge.parse("""{"id":"a1","method":"getAppInfo"}""").getOrThrow())
        assertEquals(BridgeRequest.Haptic("h"), Bridge.parse("""{"id":"h","method":"haptic","params":{}}""").getOrThrow())
        assertEquals(BridgeRequest.Share("s", "hello"), Bridge.parse("""{"id":"s","method":"share","params":{"text":"hello"}}""").getOrThrow())
        assertEquals(BridgeRequest.OpenExternal("o", "https://x.example/a"), Bridge.parse("""{"id":"o","method":"openExternal","params":{"url":"https://x.example/a"}}""").getOrThrow())
    }

    @Test
    fun everythingElseIsRefusedWithoutCrashing() {
        val refused = listOf(
            null,
            "",
            "not json",
            "[]",
            """{"method":"getAppInfo"}""",
            """{"id":"x","method":"exec","params":{"cmd":"rm -rf /"}}""",
            """{"id":"x","method":"readFile","params":{"path":"/data"}}""",
            """{"id":"x","method":"getAppInfo","extra":1}""",
            """{"id":"x","method":"getAppInfo","params":{"a":"b"}}""",
            """{"id":"bad id!","method":"getAppInfo"}""",
            """{"id":"${"a".repeat(65)}","method":"getAppInfo"}""",
            """{"id":"x","method":"share","params":{}}""",
            """{"id":"x","method":"share","params":{"text":"   "}}""",
            """{"id":"x","method":"share","params":{"text":"${"a".repeat(2001)}"}}""",
            """{"id":"x","method":"share","params":{"text":"hi","to":"someone"}}""",
            """{"id":"x","method":"share","params":{"text":{"nested":true}}}""",
            """{"id":"x","method":"openExternal","params":{"url":"http://plain.example"}}""",
            """{"id":"x","method":"openExternal","params":{"url":"javascript:alert(1)"}}""",
            """{"id":"x","method":"openExternal","params":{"url":"intent://scan#Intent;end"}}""",
            """{"id":"x","method":"openExternal","params":{"url":"file:///etc/hosts"}}""",
            "{\"id\":\"x\",\"method\":\"share\",\"params\":{\"text\":\"" + "b".repeat(9000) + "\"}}",
        )
        for (m in refused) assertTrue(m?.take(80) ?: "null", Bridge.parse(m).isFailure)
    }

    @Test
    fun projectPathsCannotLeaveTheSite() {
        assertEquals("/p/0190b2c4-1a2b-7c3d-8e4f-123456789abc", WebPolicy.projectPath("0190b2c4-1a2b-7c3d-8e4f-123456789abc"))
        for (bad in listOf("", "../x", "//evil.example", "a/b", "x?y", "https://evil.example")) {
            assertThrows(bad, IllegalArgumentException::class.java) { WebPolicy.projectPath(bad) }
        }
    }

    @Test
    fun repliesAreJson() {
        assertEquals("""{"id":"a","ok":true,"result":{"v":"1"}}""", Bridge.reply(BridgeReply("a", true, mapOf("v" to "1"))))
    }
}
