package com.tophunt.forge

import android.app.Application
import android.os.Build
import android.webkit.WebView
import com.tophunt.forge.data.Backend
import com.tophunt.forge.data.Connectivity
import com.tophunt.forge.data.DataStoreSettings
import com.tophunt.forge.data.HttpBackend
import com.tophunt.forge.data.SealedTokenStore
import com.tophunt.forge.data.SettingsStore
import com.tophunt.forge.data.SystemConnectivity

/** Everything the screens need, created once. Tests build their own with fakes. */
class AppContainer(
    val backend: Backend,
    val settings: SettingsStore,
    val connectivity: Connectivity,
    val appVersion: String,
    /** Restores the saved server before the first screen decides where to go. */
    val restore: suspend () -> Unit = {},
    /** Forgets the WebView's own session cookies (on sign-out and server change). */
    val clearWebSession: () -> Unit = {},
)

class ForgeApp : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        // Remote debugging of web content only in debug builds (threat model T9).
        WebView.setWebContentsDebuggingEnabled(WebSecurity.debuggingAllowed)
        val settings = DataStoreSettings(this)
        val backend = HttpBackend(SealedTokenStore(this), settings, BuildConfig.ALLOW_LOCAL_HTTP, deviceName = "${Build.MANUFACTURER} ${Build.MODEL}".take(100))
        container = AppContainer(backend, settings, SystemConnectivity(this), BuildConfig.VERSION_NAME, restore = backend::restore, clearWebSession = { android.webkit.CookieManager.getInstance().removeAllCookies(null) })
    }
}

object WebSecurity {
    /** Release builds can never be inspected from a computer. */
    val debuggingAllowed: Boolean get() = BuildConfig.DEBUG
}
