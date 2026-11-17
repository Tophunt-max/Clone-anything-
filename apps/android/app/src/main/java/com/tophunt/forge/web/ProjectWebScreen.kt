package com.tophunt.forge.web

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.view.HapticFeedbackConstants
import android.webkit.CookieManager
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.compose.BackHandler
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.viewinterop.AndroidView
import androidx.webkit.WebSettingsCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import com.tophunt.forge.AppContainer
import com.tophunt.forge.data.ApiException
import com.tophunt.forge.ui.ErrorPanel
import java.net.URLEncoder

private sealed interface WebState {
    data object Starting : WebState
    data object Loading : WebState
    data object Shown : WebState
    data class Failed(val error: Throwable) : WebState
}

/**
 * A project in the shared React app (screens 9–38 that live on the web), inside a WebView that
 * trusts exactly one origin. Sign-in is handed over with a single-use code in a POST body, so
 * no token is ever in a URL or readable by page scripts.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProjectWebScreen(c: AppContainer, projectId: String, title: String, onExit: () -> Unit) {
    val context = LocalContext.current
    val origin = c.backend.server?.origin
    val online by c.connectivity.online.collectAsState()
    var state by remember { mutableStateOf<WebState>(WebState.Starting) }
    var canGoBack by remember { mutableStateOf(false) }
    var attempt by remember { mutableIntStateOf(0) }
    val path = remember(projectId) { runCatching { WebPolicy.projectPath(projectId) }.getOrNull() }
    val webView = remember { mutableStateOf<WebView?>(null) }

    // Android back: history inside the web app first, then leave the screen.
    BackHandler(enabled = canGoBack) { webView.value?.goBack() }

    Scaffold(topBar = { TopAppBar(title = { Text(title.ifBlank { "Project" }) }, navigationIcon = { IconButton(onClick = onExit) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back to projects") } }) }) { pad ->
        Box(Modifier.padding(pad).fillMaxSize()) {
            if (origin == null || path == null) {
                ErrorPanel(ApiException(0, "bad_target", "This project can't be opened.", null), onRetry = null)
                return@Scaffold
            }
            AndroidView(
                modifier = Modifier.fillMaxSize().testTag("webview"),
                factory = { ctx -> secureWebView(ctx, origin, onPage = { canGoBack = it.canGoBack() }, onState = { state = it }).also { webView.value = it } },
            )
            LaunchedEffect(attempt, online) {
                val wv = webView.value ?: return@LaunchedEffect
                if (!online) {
                    state = WebState.Failed(com.tophunt.forge.data.OfflineException(java.io.IOException("offline")))
                    return@LaunchedEffect
                }
                state = WebState.Loading
                val signedIn = CookieManager.getInstance().getCookie("$origin/api/v1/auth/refresh")?.contains("forge_rt=") == true
                if (signedIn) {
                    wv.loadUrl(origin + path)
                } else {
                    try {
                        val code = c.backend.loginCode()
                        val body = "code=${URLEncoder.encode(code, "UTF-8")}&next=${URLEncoder.encode(path, "UTF-8")}"
                        wv.postUrl("$origin/api/v1/auth/exchange", body.toByteArray())
                    } catch (e: Exception) {
                        state = WebState.Failed(e)
                    }
                }
            }
            when (val s = state) {
                WebState.Starting, WebState.Loading -> LinearProgressIndicator(Modifier.testTag("web-loading"))
                is WebState.Failed -> Column(Modifier.fillMaxSize()) { ErrorPanel(s.error, onRetry = { attempt++ }) }
                WebState.Shown -> {}
            }
        }
    }
    DisposableEffect(Unit) { onDispose { webView.value?.destroy() } }
}

@SuppressLint("SetJavaScriptEnabled")
private fun secureWebView(context: Context, origin: String, onPage: (WebView) -> Unit, onState: (WebState) -> Unit): WebView {
    val wv = WebView(context)
    wv.settings.apply {
        javaScriptEnabled = true // the web app is a React app
        domStorageEnabled = true // holds the chosen workspace id only; sessions are HttpOnly cookies
        allowFileAccess = false
        allowContentAccess = false
        mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
        setSupportMultipleWindows(false)
        javaScriptCanOpenWindowsAutomatically = false
        setGeolocationEnabled(false)
    }
    if (WebViewFeature.isFeatureSupported(WebViewFeature.SAFE_BROWSING_ENABLE)) WebSettingsCompat.setSafeBrowsingEnabled(wv.settings, true)
    CookieManager.getInstance().setAcceptThirdPartyCookies(wv, false)
    wv.webViewClient = object : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            val url = request.url.toString()
            return when (WebPolicy.navigation(url, origin)) {
                WebPolicy.Nav.STAY -> false
                WebPolicy.Nav.OPEN_EXTERNALLY -> { openExternally(view.context, url); true }
                WebPolicy.Nav.BLOCK -> true
            }
        }
        override fun onPageFinished(view: WebView, url: String) {
            onPage(view)
            if (WebPolicy.originOf(url) == origin) onState(WebState.Shown)
        }
        override fun doUpdateVisitedHistory(view: WebView, url: String, isReload: Boolean) = onPage(view)
        override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
            if (request.isForMainFrame) onState(WebState.Failed(com.tophunt.forge.data.OfflineException(java.io.IOException(error.description?.toString()))))
        }
        override fun onReceivedHttpError(view: WebView, request: WebResourceRequest, response: WebResourceResponse) {
            if (request.isForMainFrame) onState(WebState.Failed(ApiException(response.statusCode, "http_error", "The project page didn't open (${response.statusCode}). Try again.", null)))
        }
    }
    // The bridge exists only for the trusted origin's main frame; nothing is injected elsewhere.
    if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
        WebViewCompat.addWebMessageListener(wv, Bridge.OBJECT_NAME, setOf(origin)) { view, message, sourceOrigin, isMainFrame, reply ->
            if (!WebPolicy.acceptsMessage(sourceOrigin.toString().trimEnd('/'), isMainFrame, origin)) return@addWebMessageListener
            val req = Bridge.parse(message.data)
            val answer = req.fold(
                onSuccess = { handle(view, it) },
                onFailure = { BridgeReply(id = "invalid", ok = false, error = it.message) },
            )
            reply.postMessage(Bridge.reply(answer))
        }
    }
    return wv
}

private fun handle(view: WebView, r: BridgeRequest): BridgeReply = when (r) {
    is BridgeRequest.AppInfo -> BridgeReply(r.id, true, mapOf("platform" to "android", "version" to com.tophunt.forge.BuildConfig.VERSION_NAME))
    is BridgeRequest.Haptic -> { view.performHapticFeedback(HapticFeedbackConstants.CONFIRM); BridgeReply(r.id, true) }
    is BridgeRequest.Share -> {
        // The system share sheet: the person picks where it goes, or cancels.
        val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, r.text)
        view.context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        BridgeReply(r.id, true)
    }
    is BridgeRequest.OpenExternal -> { openExternally(view.context, r.url); BridgeReply(r.id, true) }
}

private fun openExternally(context: Context, url: String) {
    try {
        CustomTabsIntent.Builder().build().launchUrl(context, Uri.parse(url))
    } catch (_: ActivityNotFoundException) {
        // No browser: nothing opens, and nothing else is tried.
    }
}
