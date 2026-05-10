// SPDX-License-Identifier: Apache-2.0

package dev.aptu.android.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import dev.aptu.shared.viewmodels.AuthViewModel
import dev.aptu.shared.viewmodels.IssueViewModel
import dev.aptu.shared.viewmodels.RepoViewModel

private val DarkColorScheme = darkColorScheme()
private val LightColorScheme = lightColorScheme()

@Composable
fun AptuTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    val colorScheme = if (darkTheme) DarkColorScheme else LightColorScheme
    MaterialTheme(
        colorScheme = colorScheme,
        content = content,
    )
}

// ViewModels are plain KMP classes (not AndroidX ViewModel) so they live in
// commonMain and share state logic across platforms. They are created once at the
// NavHost level via remember {} so they survive recomposition. This is intentional:
// process death and configuration changes are handled by the screens re-triggering
// load() on reentry. A DI framework (e.g. Koin Multiplatform) can replace remember {}
// here when the app grows beyond this scaffold -- the ViewModel interfaces stay the same.
@Composable
fun AppNavHost(navController: NavHostController = rememberNavController()) {
    val authViewModel = remember { AuthViewModel() }
    val repoViewModel = remember { RepoViewModel() }
    val issueViewModel = remember { IssueViewModel() }

    NavHost(
        navController = navController,
        startDestination = "auth",
    ) {
        composable("auth") {
            AuthScreen(
                viewModel = authViewModel,
                onAuthSuccess = {
                    navController.navigate("repos") {
                        popUpTo("auth") { inclusive = true }
                    }
                },
            )
        }

        composable("repos") {
            RepoPickerScreen(
                viewModel = repoViewModel,
                onRepoSelected = { owner, name ->
                    val encodedOwner = URLEncoder.encode(owner, StandardCharsets.UTF_8.name())
                    val encodedName = URLEncoder.encode(name, StandardCharsets.UTF_8.name())
                    navController.navigate("issues/$encodedOwner/$encodedName")
                },
                onNavigateToSettings = {
                    navController.navigate("settings")
                },
            )
        }

        composable("issues/{owner}/{repo}") { backStackEntry ->
            val owner = backStackEntry.arguments?.getString("owner") ?: ""
            val repo = backStackEntry.arguments?.getString("repo") ?: ""
            IssueListScreen(
                owner = owner,
                repo = repo,
                viewModel = issueViewModel,
                onIssueSelected = { issueId ->
                    navController.navigate("issue_detail/$issueId")
                },
                onNavigateBack = {
                    navController.popBackStack()
                },
            )
        }

        composable("issue_detail/{issueId}") { backStackEntry ->
            val issueId = backStackEntry.arguments?.getString("issueId") ?: ""
            IssueDetailScreen(
                issueId = issueId,
                onNavigateBack = {
                    navController.popBackStack()
                },
            )
        }

        composable("settings") {
            SettingsScreen(
                onNavigateBack = {
                    navController.popBackStack()
                },
                onLogout = {
                    navController.navigate("auth") {
                        popUpTo("settings") { inclusive = true }
                    }
                },
            )
        }
    }
}
