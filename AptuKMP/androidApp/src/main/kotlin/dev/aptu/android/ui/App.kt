// SPDX-License-Identifier: Apache-2.0

package dev.aptu.android.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController

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

@Composable
fun AppNavHost(navController: NavHostController = rememberNavController()) {
    NavHost(
        navController = navController,
        startDestination = "auth",
    ) {
        composable("auth") {
            AuthScreen(
                onAuthSuccess = {
                    navController.navigate("repos") {
                        popUpTo("auth") { inclusive = true }
                    }
                },
            )
        }

        composable("repos") {
            RepoPickerScreen(
                onRepoSelected = { owner, name ->
                    navController.navigate("issues/$owner/$name")
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
