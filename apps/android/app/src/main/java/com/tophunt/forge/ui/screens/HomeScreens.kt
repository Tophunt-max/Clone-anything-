package com.tophunt.forge.ui.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.Settings
import com.tophunt.forge.AppContainer
import com.tophunt.forge.data.Project
import com.tophunt.forge.data.Workspace
import com.tophunt.forge.ui.Centered
import com.tophunt.forge.ui.LoadView
import com.tophunt.forge.ui.OfflineBanner
import com.tophunt.forge.ui.rememberLoad
import com.tophunt.forge.ui.theme.ForgeTokens
import kotlinx.coroutines.launch

private enum class Tab(val label: String, val icon: ImageVector) { PROJECTS("Projects", Icons.AutoMirrored.Filled.List), SETTINGS("Settings", Icons.Filled.Settings) }

/** Screen 6 (dashboard shell) with screen 7 (project list) and native settings. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(c: AppContainer, onOpenProject: (Project) -> Unit, onSignedOut: () -> Unit, onChangeServer: () -> Unit, onDiagnostics: () -> Unit) {
    var tab by rememberSaveable { mutableStateOf(Tab.PROJECTS) }
    val online by c.connectivity.online.collectAsState()
    Scaffold(
        topBar = { TopAppBar(title = { Text(tab.label) }) },
        bottomBar = {
            NavigationBar {
                Tab.entries.forEach { t -> NavigationBarItem(selected = tab == t, onClick = { tab = t }, icon = { Icon(t.icon, contentDescription = null) }, label = { Text(t.label) }) }
            }
        },
    ) { pad ->
        Column(Modifier.padding(pad).fillMaxSize()) {
            if (!online) OfflineBanner()
            when (tab) {
                Tab.PROJECTS -> ProjectsTab(c, online, onOpenProject)
                Tab.SETTINGS -> SettingsTab(c, onSignedOut, onChangeServer, onDiagnostics)
            }
        }
    }
}

@Composable
private fun ProjectsTab(c: AppContainer, online: Boolean, onOpenProject: (Project) -> Unit) {
    val workspaces = rememberLoad(online) { c.backend.workspaces() }
    LoadView(workspaces, "Loading workspaces…", isEmpty = { it.isEmpty() }, empty = { Centered { Text("You aren't in any workspace.") } }) { list ->
        var chosen by rememberSaveable { mutableStateOf(list.first().id) }
        val ws = list.firstOrNull { it.id == chosen } ?: list.first()
        Column {
            WorkspacePicker(list, ws) { chosen = it.id }
            val projects = rememberLoad(ws.id, online) { c.backend.projects(ws.id).filter { it.archivedAt == null } }
            LoadView(
                projects,
                "Loading projects…",
                isEmpty = { it.isEmpty() },
                empty = { Centered { Text("No projects yet", style = MaterialTheme.typography.titleMedium); Text("Create one in the web app; it shows up here.", color = MaterialTheme.colorScheme.onSurfaceVariant) } },
            ) { items ->
                LazyColumn(contentPadding = androidx.compose.foundation.layout.PaddingValues(ForgeTokens.Space.lg), verticalArrangement = Arrangement.spacedBy(ForgeTokens.Space.sm)) {
                    items(items, key = { it.id }) { p ->
                        Card(Modifier.fillMaxWidth().clickable { onOpenProject(p) }) {
                            Column(Modifier.padding(ForgeTokens.Space.lg)) {
                                Text(p.name, style = MaterialTheme.typography.titleMedium)
                                Text(p.kind, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun WorkspacePicker(list: List<Workspace>, current: Workspace, onPick: (Workspace) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Column(Modifier.padding(horizontal = ForgeTokens.Space.lg)) {
        TextButton(onClick = { open = true }, enabled = list.size > 1) { Text("Workspace: ${current.name} (${current.role})") }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            list.forEach { w -> DropdownMenuItem(text = { Text(w.name) }, onClick = { open = false; onPick(w) }) }
        }
    }
}

@Composable
private fun SettingsTab(c: AppContainer, onSignedOut: () -> Unit, onChangeServer: () -> Unit, onDiagnostics: () -> Unit) {
    val scope = rememberCoroutineScope()
    val me = rememberLoad { c.backend.me() }
    Column(Modifier.verticalScroll(rememberScrollState()).padding(ForgeTokens.Space.lg), verticalArrangement = Arrangement.spacedBy(ForgeTokens.Space.md)) {
        Text("Account", style = MaterialTheme.typography.titleMedium)
        LoadView(me, "Loading your account…") { u ->
            Column {
                Text(u.displayName)
                Text(u.email, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        Text("Server", style = MaterialTheme.typography.titleMedium)
        Text(c.backend.server?.origin ?: "none", fontFamily = FontFamily.Monospace)
        OutlinedButton(onClick = onChangeServer) { Text("Change server") }
        OutlinedButton(onClick = onDiagnostics) { Text("Help and diagnostics") }
        Button(onClick = {
            scope.launch {
                c.backend.logout()
                // The WebView's own session cookies go too.
                c.clearWebSession()
                onSignedOut()
            }
        }) { Text("Sign out") }
        Text("Signed-in devices, members and workspace settings are in each project's web view.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Screen 38, native part: works before sign-in and without a server. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DiagnosticsScreen(c: AppContainer, onBack: () -> Unit) {
    val online by c.connectivity.online.collectAsState()
    val server = c.backend.server
    val meta = rememberLoad(server, online) { if (server == null) null else c.backend.meta() }
    val ready = rememberLoad(server, online) { server != null && c.backend.ready() }
    Scaffold(topBar = { TopAppBar(title = { Text("Help and diagnostics") }, navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } }) }) { pad ->
        Column(Modifier.padding(pad).padding(ForgeTokens.Space.lg), verticalArrangement = Arrangement.spacedBy(ForgeTokens.Space.md)) {
            Row("App version", c.appVersion)
            Row("Network", if (online) "online" else "offline")
            Row("Server", server?.origin ?: "not chosen")
            LoadView(meta, "Checking the server…") { m -> Row("Server version", m?.let { "${it.name} ${it.version}, API ${it.apiVersion}" } ?: "—") }
            LoadView(ready, "Checking readiness…") { r -> Row("Server ready", if (r) "yes" else "no (database unreachable or migrations pending)") }
        }
    }
}

@Composable
private fun Row(label: String, value: String) {
    Column {
        Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value, style = MaterialTheme.typography.bodyLarge)
    }
}
