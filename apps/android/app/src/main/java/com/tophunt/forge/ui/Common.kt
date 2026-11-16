package com.tophunt.forge.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import com.tophunt.forge.data.ApiException
import com.tophunt.forge.data.OfflineException
import com.tophunt.forge.ui.theme.ForgeTokens

/** Loading → ready or failed, for one request. */
sealed interface Load<out T> {
    data object Loading : Load<Nothing>
    data class Ready<T>(val value: T) : Load<T>
    data class Failed(val error: Throwable) : Load<Nothing>
}

/** Runs [fetch] when the key changes or [retry] is called. */
class Loader<T>(val state: Load<T>, val retry: () -> Unit)

@Composable
fun <T> rememberLoad(vararg keys: Any?, fetch: suspend () -> T): Loader<T> {
    var attempt by remember { mutableIntStateOf(0) }
    var state by remember(*keys) { mutableStateOf<Load<T>>(Load.Loading) }
    LaunchedEffect(*keys, attempt) {
        state = Load.Loading
        state = try {
            Load.Ready(fetch())
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            Load.Failed(e)
        }
    }
    return Loader(state) { attempt++ }
}

/** The four states every data screen shows: loading, error (with retry), empty, ready. */
@Composable
fun <T> LoadView(loader: Loader<T>, loadingText: String, isEmpty: (T) -> Boolean = { false }, empty: @Composable () -> Unit = {}, content: @Composable (T) -> Unit) {
    when (val s = loader.state) {
        Load.Loading -> Centered {
            CircularProgressIndicator()
            Text(loadingText, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        is Load.Failed -> ErrorPanel(s.error, onRetry = loader.retry)
        is Load.Ready -> if (isEmpty(s.value)) empty() else content(s.value)
    }
}

@Composable
fun Centered(content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(ForgeTokens.Space.xl), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(ForgeTokens.Space.md)) {
        content()
    }
}

data class ErrorText(val title: String, val message: String, val requestId: String?)

fun describe(e: Throwable): ErrorText = when (e) {
    is OfflineException -> ErrorText("You're offline", e.message ?: "Can't reach the server.", null)
    is ApiException -> ErrorText(if (e.status == 403) "You don't have access" else "Something went wrong", e.message, e.requestId)
    else -> ErrorText("Something went wrong", e.message ?: "Unexpected error.", null)
}

@Composable
fun ErrorPanel(error: Throwable, onRetry: (() -> Unit)?) {
    val t = describe(error)
    Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = MaterialTheme.shapes.medium, modifier = Modifier.fillMaxWidth().padding(ForgeTokens.Space.lg).testTag("error")) {
        Column(Modifier.padding(ForgeTokens.Space.lg), verticalArrangement = Arrangement.spacedBy(ForgeTokens.Space.sm)) {
            Text(t.title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.error)
            Text(t.message, style = MaterialTheme.typography.bodyMedium)
            t.requestId?.let { Text("Request ID: $it", style = MaterialTheme.typography.labelSmall, fontFamily = FontFamily.Monospace) }
            if (onRetry != null) Button(onClick = onRetry) { Text("Try again") }
        }
    }
}

@Composable
fun OfflineBanner() {
    Surface(color = MaterialTheme.colorScheme.errorContainer, modifier = Modifier.fillMaxWidth().testTag("offline")) {
        Text("You're offline. Showing what can be shown without the server.", modifier = Modifier.padding(ForgeTokens.Space.md), style = MaterialTheme.typography.bodyMedium)
    }
}
