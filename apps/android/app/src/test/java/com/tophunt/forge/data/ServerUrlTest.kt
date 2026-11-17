package com.tophunt.forge.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ServerUrlTest {
    private fun ok(input: String, local: Boolean = false) = ServerUrl.parse(input, local).getOrThrow().origin
    private fun bad(input: String, local: Boolean = false) = assertTrue(input, ServerUrl.parse(input, local).isFailure)

    @Test
    fun reducesToAnHttpsOrigin() {
        assertEquals("https://forge.example.com", ok("forge.example.com"))
        assertEquals("https://forge.example.com", ok("  HTTPS://Forge.Example.com/  "))
        assertEquals("https://forge.example.com:8443", ok("https://forge.example.com:8443"))
        assertEquals("https://forge.example.com", ok("https://forge.example.com:443"))
    }

    @Test
    fun refusesPlainHttpExceptLocalHostsInDebug() {
        bad("http://forge.example.com")
        bad("http://forge.example.com", local = true)
        bad("http://10.0.2.2:8080")
        assertEquals("http://10.0.2.2:8080", ok("http://10.0.2.2:8080", local = true))
    }

    @Test
    fun refusesAnythingButABareOrigin() {
        for (s in listOf("", "https://", "ftp://x.com", "javascript:alert(1)", "file:///etc/passwd", "https://u:p@x.com", "https://x.com/path", "https://x.com?q=1", "https://x.com#f", "https://x .com")) bad(s)
    }

    @Test
    fun storedValuesAreCheckedAgain() {
        assertEquals(null, ServerUrl.restore("http://evil.example", allowLocalHttp = false))
        assertEquals("https://a.example", ServerUrl.restore("https://a.example", allowLocalHttp = false)?.origin)
    }
}
