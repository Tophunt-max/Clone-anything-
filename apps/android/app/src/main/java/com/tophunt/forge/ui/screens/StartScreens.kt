package com.tophunt.forge.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import com.tophunt.forge.AppContainer
import com.tophunt.forge.BuildConfig
import com.tophunt.forge.data.ApiException
import com.tophunt.forge.data.Backend
import com.tophunt.forge.data.ServerUrl
import com.tophunt.forge.ui.ErrorPanel
import com.tophunt.forge.ui.Routes
import com.tophunt.forge.ui.theme.ForgeTokens
import kotlinx.coroutines.launch

@Composable
fun Page(content: @Composable () -> Unit) {
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        Column(
            Modifier.safeDrawingPadding().imePadding().verticalScroll(rememberScrollState()).padding(ForgeTokens.Space.xl),
            verticalArrangement = Arrangement.spacedBy(ForgeTokens.Space.lg),
        ) { content() }
    }
}

/** Screen 1: decides where to start, and says why when it can't. */
@Composable
fun SplashScreen(c: AppContainer, go: (String) -> Unit) {
    var failure by remember { mutableStateOf<Throwable?>(null) }
    var attempt by remember { mutableIntStateOf(0) }
    LaunchedEffect(attempt) {
        failure = null
        c.restore()
        val next = when {
            !c.settings.onboardingDone() -> Routes.ONBOARDING
            c.backend.server == null -> Routes.SERVER
            !c.backend.hasSession() -> Routes.LOGIN
            else -> try {
                c.backend.me()
                Routes.HOME
            } catch (e: ApiException) {
                if (e.status == 401) Routes.LOGIN else { failure = e; null }
            } catch (e: Exception) {
                failure = e
                null
            }
        }
        next?.let(go)
    }
    Page {
        Text("Tophunt Forge", style = MaterialTheme.typography.headlineMedium)
        val f = failure
        if (f == null) {
            CircularProgressIndicator()
        } else {
            ErrorPanel(f, onRetry = { attempt++ })
            TextButton(onClick = { go(Routes.SERVER) }) { Text("Change server") }
        }
    }
}

private val PAGES = listOf(
    "Build with AI agents" to "Plan, write and review code with agents that ask before they act.",
    "Analyze with evidence" to "Study apps, websites and files you are allowed to analyze. Every finding points to its evidence.",
    "Your server, your data" to "The app connects to a Tophunt Forge server you choose. Sign-in stays on this device, encrypted.",
)

/** Screen 2. */
@Composable
fun OnboardingScreen(onDone: () -> Unit) {
    var page by remember { mutableIntStateOf(0) }
    val (title, body) = PAGES[page]
    Page {
        Text("${page + 1} of ${PAGES.size}", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(title, style = MaterialTheme.typography.headlineMedium)
        Text(body, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = onDone) { Text("Skip") }
            if (page < PAGES.lastIndex) Button(onClick = { page++ }) { Text("Next") } else Button(onClick = onDone) { Text("Get started") }
        }
    }
}

/** Screen 3: checks the server answers as a Tophunt Forge API before saving it. */
@Composable
fun ServerScreen(c: AppContainer, onConnected: () -> Unit, onDiagnostics: () -> Unit) {
    val scope = rememberCoroutineScope()
    var text by remember { mutableStateOf(c.backend.server?.origin ?: "") }
    var fieldError by remember { mutableStateOf<String?>(null) }
    var failure by remember { mutableStateOf<Throwable?>(null) }
    var busy by remember { mutableStateOf(false) }
    val connect = {
        fieldError = null
        failure = null
        ServerUrl.parse(text, BuildConfig.ALLOW_LOCAL_HTTP).fold(
            onSuccess = { url ->
                busy = true
                scope.launch {
                    try {
                        val changed = c.backend.server != url
                        c.backend.connect(url)
                        if (changed) c.clearWebSession()
                        onConnected()
                    } catch (e: Exception) {
                        failure = e
                    } finally {
                        busy = false
                    }
                }
            },
            onFailure = { fieldError = it.message },
        )
        Unit
    }
    Page {
        Text("Connect to a server", style = MaterialTheme.typography.headlineMedium)
        Text("Enter the address your team uses, for example forge.example.com.", color = MaterialTheme.colorScheme.onSurfaceVariant)
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            label = { Text("Server address") },
            singleLine = true,
            isError = fieldError != null,
            supportingText = fieldError?.let { { Text(it) } },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
            modifier = Modifier.fillMaxWidth(),
        )
        failure?.let { ErrorPanel(it, onRetry = null) }
        Button(onClick = connect, enabled = text.isNotBlank() && !busy, modifier = Modifier.fillMaxWidth()) { Text(if (busy) "Connecting…" else "Connect") }
        TextButton(onClick = onDiagnostics) { Text("Help and diagnostics") }
    }
}

@Composable
private fun AuthForm(
    title: String,
    busy: Boolean,
    failure: Throwable?,
    submitLabel: String,
    canSubmit: Boolean,
    onSubmit: () -> Unit,
    fields: @Composable () -> Unit,
    footer: @Composable () -> Unit,
) {
    Page {
        Text(title, style = MaterialTheme.typography.headlineMedium)
        fields()
        failure?.let { ErrorPanel(it, onRetry = null) }
        Button(onClick = onSubmit, enabled = canSubmit && !busy, modifier = Modifier.fillMaxWidth()) { Text(if (busy) "Please wait…" else submitLabel) }
        footer()
    }
}

/** Screen 4. */
@Composable
fun LoginScreen(backend: Backend, onSignedIn: () -> Unit, onRegister: () -> Unit, onChangeServer: () -> Unit) {
    val scope = rememberCoroutineScope()
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var failure by remember { mutableStateOf<Throwable?>(null) }
    AuthForm(
        title = "Sign in",
        busy = busy,
        failure = failure,
        submitLabel = "Sign in",
        canSubmit = email.isNotBlank() && password.isNotEmpty(),
        onSubmit = {
            busy = true
            failure = null
            scope.launch {
                try {
                    backend.login(email, password)
                    onSignedIn()
                } catch (e: Exception) {
                    failure = e
                } finally {
                    busy = false
                }
            }
        },
        fields = {
            OutlinedTextField(email, { email = it }, label = { Text("Email") }, singleLine = true, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email), modifier = Modifier.fillMaxWidth())
            OutlinedTextField(password, { password = it }, label = { Text("Password") }, singleLine = true, visualTransformation = PasswordVisualTransformation(), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password), modifier = Modifier.fillMaxWidth())
        },
        footer = {
            TextButton(onClick = onRegister) { Text("Create an account") }
            TextButton(onClick = onChangeServer) { Text("Server: ${backend.server?.origin ?: "none"} · Change") }
        },
    )
}

/** Screen 5. */
@Composable
fun RegisterScreen(backend: Backend, onSignedIn: () -> Unit, onBack: () -> Unit) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var failure by remember { mutableStateOf<Throwable?>(null) }
    val tooShort = password.isNotEmpty() && password.codePointCount(0, password.length) < 12
    val serverField = (failure as? ApiException)?.field
    AuthForm(
        title = "Create an account",
        busy = busy,
        failure = failure.takeIf { serverField == null },
        submitLabel = "Create account",
        canSubmit = name.isNotBlank() && email.isNotBlank() && password.isNotEmpty() && !tooShort,
        onSubmit = {
            busy = true
            failure = null
            scope.launch {
                try {
                    backend.register(name, email, password)
                    onSignedIn()
                } catch (e: Exception) {
                    failure = e
                } finally {
                    busy = false
                }
            }
        },
        fields = {
            fun err(f: String) = failure?.message.takeIf { serverField == f }
            OutlinedTextField(name, { name = it }, label = { Text("Your name") }, singleLine = true, isError = err("display_name") != null, supportingText = err("display_name")?.let { { Text(it) } }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(email, { email = it }, label = { Text("Email") }, singleLine = true, isError = err("email") != null, supportingText = err("email")?.let { { Text(it) } }, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email), modifier = Modifier.fillMaxWidth())
            OutlinedTextField(
                password,
                { password = it },
                label = { Text("Password") },
                singleLine = true,
                isError = tooShort || err("password") != null,
                supportingText = { Text(if (tooShort) "Use at least 12 characters." else err("password") ?: "At least 12 characters.") },
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                modifier = Modifier.fillMaxWidth(),
            )
        },
        footer = { TextButton(onClick = onBack) { Text("I have an account") } },
    )
}
