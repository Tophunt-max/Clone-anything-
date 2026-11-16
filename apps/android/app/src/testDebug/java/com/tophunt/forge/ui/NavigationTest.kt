package com.tophunt.forge.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.performScrollTo
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.tophunt.forge.AppContainer
import com.tophunt.forge.data.ApiException
import com.tophunt.forge.data.Backend
import com.tophunt.forge.data.FixedConnectivity
import com.tophunt.forge.data.InMemorySettingsStore
import com.tophunt.forge.data.Meta
import com.tophunt.forge.data.OfflineException
import com.tophunt.forge.data.Project
import com.tophunt.forge.data.ServerUrl
import com.tophunt.forge.data.User
import com.tophunt.forge.data.Workspace
import com.tophunt.forge.ui.theme.ForgeTheme
import kotlinx.coroutines.flow.MutableSharedFlow
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

// Compose UI tests need the debug-only test manifest (ui-test-manifest), so they live in testDebug.

/** A scripted server for the native screens. */
class FakeBackend(var projectList: List<Project> = emptyList(), var signedIn: Boolean = false) : Backend {
    override var server: ServerUrl? = null
    override val sessionEnded = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    var projectsError: Throwable? = null
    var loggedOut = false
    private val ada = User("u1", "ada@example.com", "Ada")

    override suspend fun connect(server: ServerUrl): Meta {
        if (server.origin.contains("down")) throw OfflineException(java.io.IOException("no route"))
        this.server = server
        return Meta("Tophunt Forge", "0.1.0", "v1")
    }
    override suspend fun meta() = Meta("Tophunt Forge", "0.1.0", "v1")
    override suspend fun hasSession() = signedIn
    override suspend fun me(): User = if (signedIn) ada else throw ApiException(401, "unauthenticated", "Sign in again.", "r")
    override suspend fun login(email: String, password: String): User {
        if (password != "a long enough passphrase") throw ApiException(401, "invalid_credentials", "The email or password is wrong.", "rid-42")
        signedIn = true
        return ada
    }
    override suspend fun register(name: String, email: String, password: String): User = login(email, password)
    override suspend fun logout() { signedIn = false; loggedOut = true }
    override suspend fun workspaces() = listOf(Workspace("w1", "Ada's workspace", "personal", "owner"))
    override suspend fun projects(workspace: String): List<Project> = projectsError?.let { throw it } ?: projectList
    override suspend fun loginCode() = "flc1.test"
    override suspend fun ready() = true
}

@RunWith(AndroidJUnit4::class)
@Config(sdk = [34])
class NavigationTest {
    @get:Rule val rule = createComposeRule()

    private lateinit var nav: NavHostController

    private fun launch(backend: FakeBackend, settings: InMemorySettingsStore = InMemorySettingsStore(), online: Boolean = true): AppContainer {
        val c = AppContainer(backend, settings, FixedConnectivity(online), appVersion = "0.1.0-test")
        rule.setContent {
            nav = rememberNavController()
            ForgeTheme(dark = true) { ForgeNavHost(c, nav) }
        }
        return c
    }

    private fun route() = nav.currentDestination?.route

    @Test
    fun firstLaunchGoesOnboardingThenServerThenSignIn() {
        launch(FakeBackend())
        rule.onNodeWithText("Build with AI agents").assertIsDisplayed()
        rule.onNodeWithText("Next").performClick()
        rule.onNodeWithText("Analyze with evidence").assertIsDisplayed()
        rule.onNodeWithText("Skip").performClick()
        rule.waitUntil(3_000) { route() == Routes.SERVER }

        rule.onNodeWithText("Server address").performTextInput("http://forge.example.com")
        rule.onNodeWithText("Connect").performClick()
        rule.onNodeWithText("The server must use HTTPS.").assertIsDisplayed()
        assertEquals(Routes.SERVER, route())
    }

    @Test
    fun anUnreachableServerSaysSoAndStays() {
        launch(FakeBackend(), InMemorySettingsStore(onboarded = true))
        rule.waitUntil(3_000) { route() == Routes.SERVER }
        rule.onNodeWithText("Server address").performTextInput("down.example.com")
        rule.onNodeWithText("Connect").performClick()
        rule.waitUntil(3_000) { rule.onAllNodes(hasText("You're offline")).fetchSemanticsNodes().isNotEmpty() }
        assertEquals(Routes.SERVER, route())
    }

    @Test
    fun signInShowsServerErrorsThenOpensProjects() {
        val backend = FakeBackend(projectList = listOf(Project("0190b2c4-0000-7000-8000-000000000001", "w1", "Weather app", "code", "now")))
        launch(backend, InMemorySettingsStore(onboarded = true))
        rule.waitUntil(3_000) { route() == Routes.SERVER }
        rule.onNodeWithText("Server address").performTextInput("forge.example.com")
        rule.onNodeWithText("Connect").performClick()
        rule.waitUntil(3_000) { route() == Routes.LOGIN }

        rule.onNodeWithText("Email").performTextInput("ada@example.com")
        rule.onNodeWithText("Password").performTextInput("wrong")
        rule.onAllNodes(hasText("Sign in") and hasClickAction())[0].performClick()
        rule.waitUntil(3_000) { rule.onAllNodes(hasText("Request ID: rid-42")).fetchSemanticsNodes().isNotEmpty() }
        rule.onNodeWithText("The email or password is wrong.").assertIsDisplayed()

        rule.onNodeWithText("Password").performTextClearance()
        rule.onNodeWithText("Password").performTextInput("a long enough passphrase")
        rule.onAllNodes(hasText("Sign in") and hasClickAction())[0].performClick()
        rule.waitUntil(3_000) { route() == Routes.HOME }
        rule.waitUntil(3_000) { rule.onAllNodes(hasText("Weather app")).fetchSemanticsNodes().isNotEmpty() }
        // Back stack was reset: there is no way "back" into the sign-in screen.
        assertEquals(null, nav.previousBackStackEntry)
    }

    @Test
    fun aSavedSessionGoesStraightHomeAndShowsTheEmptyState() {
        val backend = FakeBackend(signedIn = true).apply { server = ServerUrl.parse("forge.example.com", false).getOrThrow() }
        launch(backend, InMemorySettingsStore(server = "https://forge.example.com", onboarded = true))
        rule.waitUntil(3_000) { route() == Routes.HOME }
        rule.waitUntil(3_000) { rule.onAllNodes(hasText("No projects yet")).fetchSemanticsNodes().isNotEmpty() }
    }

    @Test
    fun offlineShowsTheBannerAndAnErrorWithRetry() {
        val backend = FakeBackend(signedIn = true).apply {
            server = ServerUrl.parse("forge.example.com", false).getOrThrow()
            projectsError = OfflineException(java.io.IOException("down"))
        }
        launch(backend, InMemorySettingsStore(server = "https://forge.example.com", onboarded = true), online = false)
        rule.waitUntil(3_000) { route() == Routes.HOME }
        rule.onNodeWithTag("offline").assertIsDisplayed()
        rule.waitUntil(3_000) { rule.onAllNodes(hasText("Try again")).fetchSemanticsNodes().isNotEmpty() }
        backend.projectsError = null
        backend.projectList = listOf(Project("0190b2c4-0000-7000-8000-000000000002", "w1", "Back online", "code", "now"))
        rule.onNodeWithText("Try again").performClick()
        rule.waitUntil(3_000) { rule.onAllNodes(hasText("Back online")).fetchSemanticsNodes().isNotEmpty() }
    }

    @Test
    fun signOutAndAnEndedSessionReturnToSignIn() {
        val backend = FakeBackend(signedIn = true).apply { server = ServerUrl.parse("forge.example.com", false).getOrThrow() }
        launch(backend, InMemorySettingsStore(server = "https://forge.example.com", onboarded = true))
        rule.waitUntil(3_000) { route() == Routes.HOME }
        rule.onNodeWithText("Settings").performClick()
        rule.waitUntil(3_000) { rule.onAllNodes(hasText("ada@example.com")).fetchSemanticsNodes().isNotEmpty() }
        // The button is below the fold on a small screen: scroll like a person would.
        rule.onNodeWithText("Sign out").performScrollTo().performClick()
        rule.waitUntil(3_000) { route() == Routes.LOGIN }
        assertEquals(true, backend.loggedOut)

        // A refused refresh elsewhere also lands on sign-in.
        rule.runOnUiThread { nav.reset(Routes.HOME) }
        backend.signedIn = true
        rule.waitUntil(3_000) { route() == Routes.HOME }
        backend.sessionEnded.tryEmit(Unit)
        rule.waitUntil(3_000) { route() == Routes.LOGIN }
    }

    @Test
    fun registerAndBackReturnToSignIn() {
        val backend = FakeBackend().apply { server = ServerUrl.parse("forge.example.com", false).getOrThrow() }
        launch(backend, InMemorySettingsStore(server = "https://forge.example.com", onboarded = true))
        rule.waitUntil(3_000) { route() == Routes.LOGIN }
        rule.onNodeWithText("Create an account").performClick()
        rule.waitUntil(3_000) { route() == Routes.REGISTER }
        rule.onNodeWithText("Password").performTextInput("short")
        rule.onNodeWithText("Use at least 12 characters.").assertIsDisplayed()
        rule.onNodeWithText("I have an account").performClick()
        rule.waitUntil(3_000) { route() == Routes.LOGIN }
    }
}

