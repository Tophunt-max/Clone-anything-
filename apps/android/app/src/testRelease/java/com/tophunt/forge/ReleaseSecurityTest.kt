package com.tophunt.forge

import org.junit.Assert.assertFalse
import org.junit.Test

/** Runs only against the release build type (`testReleaseUnitTest`). */
class ReleaseSecurityTest {
    @Test
    fun releaseBuildsCannotBeDebuggedOrUsePlainHttp() {
        assertFalse("WebView remote debugging must be off in release", WebSecurity.debuggingAllowed)
        assertFalse("release builds must not allow local plain HTTP", BuildConfig.ALLOW_LOCAL_HTTP)
        assertFalse(BuildConfig.DEBUG)
    }
}
