package com.tophunt.forge.ui

import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.tophunt.forge.AppContainer
import com.tophunt.forge.ui.screens.DiagnosticsScreen
import com.tophunt.forge.ui.screens.HomeScreen
import com.tophunt.forge.ui.screens.LoginScreen
import com.tophunt.forge.ui.screens.OnboardingScreen
import com.tophunt.forge.ui.screens.RegisterScreen
import com.tophunt.forge.ui.screens.ServerScreen
import com.tophunt.forge.ui.screens.SplashScreen
import com.tophunt.forge.web.ProjectWebScreen
import kotlinx.coroutines.launch

object Routes {
    const val SPLASH = "splash"
    const val ONBOARDING = "onboarding"
    const val SERVER = "server"
    const val LOGIN = "login"
    const val REGISTER = "register"
    const val HOME = "home"
    const val DIAGNOSTICS = "diagnostics"
    const val PROJECT = "project/{id}?name={name}"
    fun project(id: String, name: String) = "project/${Uri.encode(id)}?name=${Uri.encode(name)}"
}

/** Replace the whole back stack (after sign-in, sign-out, server change). */
fun NavHostController.reset(route: String) = navigate(route) { popUpTo(0) { inclusive = true }; launchSingleTop = true }

@Composable
fun ForgeNavHost(container: AppContainer, nav: NavHostController = rememberNavController()) {
    // A refused refresh anywhere ends the session: back to sign-in.
    LaunchedEffect(container.backend) { container.backend.sessionEnded.collect { nav.reset(Routes.LOGIN) } }
    NavHost(nav, startDestination = Routes.SPLASH) {
        composable(Routes.SPLASH) { SplashScreen(container, go = nav::reset) }
        composable(Routes.ONBOARDING) {
            val scope = rememberCoroutineScope()
            OnboardingScreen(onDone = { scope.launch { container.settings.setOnboardingDone(); nav.reset(Routes.SERVER) } })
        }
        composable(Routes.SERVER) { ServerScreen(container, onConnected = { nav.reset(Routes.LOGIN) }, onDiagnostics = { nav.navigate(Routes.DIAGNOSTICS) }) }
        composable(Routes.LOGIN) { LoginScreen(container.backend, onSignedIn = { nav.reset(Routes.HOME) }, onRegister = { nav.navigate(Routes.REGISTER) }, onChangeServer = { nav.navigate(Routes.SERVER) }) }
        composable(Routes.REGISTER) { RegisterScreen(container.backend, onSignedIn = { nav.reset(Routes.HOME) }, onBack = { nav.popBackStack() }) }
        composable(Routes.HOME) {
            HomeScreen(
                container,
                onOpenProject = { p -> nav.navigate(Routes.project(p.id, p.name)) },
                onSignedOut = { nav.reset(Routes.LOGIN) },
                onChangeServer = { nav.navigate(Routes.SERVER) },
                onDiagnostics = { nav.navigate(Routes.DIAGNOSTICS) },
            )
        }
        composable(Routes.DIAGNOSTICS) { DiagnosticsScreen(container, onBack = { nav.popBackStack() }) }
        composable(Routes.PROJECT, arguments = listOf(navArgument("id") { type = NavType.StringType }, navArgument("name") { type = NavType.StringType; defaultValue = "" })) { entry ->
            ProjectWebScreen(container, projectId = entry.arguments?.getString("id").orEmpty(), title = entry.arguments?.getString("name").orEmpty(), onExit = { nav.popBackStack() })
        }
    }
}
