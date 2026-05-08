// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

import dev.aptu.shared.models.Issue
import dev.aptu.shared.models.Repo
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

sealed class AptuError : Exception() {
    data class IOException(val message: String) : AptuError()
    data class AuthError(val message: String) : AptuError()
    data class NetworkError(val message: String) : AptuError()
}

object AptuFfi {
    suspend fun listCuratedRepos(): List<Repo> = withContext(Dispatchers.IO) {
        try {
            // TODO: Call UniFFI-generated listCuratedRepos() function once Gobley generates bindings
            // Expected signature from lib.rs: pub fn list_curated_repos() -> Vec<Repo>
            // For now, return empty list as placeholder
            emptyList()
        } catch (e: Exception) {
            throw AptuError.IOException("Failed to list curated repos: ${e.message}")
        }
    }

    suspend fun fetchIssues(keychain: AptuKeychain): List<Issue> = withContext(Dispatchers.IO) {
        try {
            // TODO: Call UniFFI-generated fetchIssues(keychain) function once Gobley generates bindings
            // Expected signature from lib.rs: pub fn fetch_issues(keychain: KeychainProvider) -> Vec<Issue>
            // For now, return empty list as placeholder
            emptyList()
        } catch (e: Exception) {
            throw AptuError.NetworkError("Failed to fetch issues: ${e.message}")
        }
    }

    suspend fun checkAuthStatus(keychain: AptuKeychain): Boolean = withContext(Dispatchers.IO) {
        try {
            // TODO: Call UniFFI-generated checkAuthStatus(keychain) function once Gobley generates bindings
            // Expected signature from lib.rs: pub fn check_auth_status(keychain: KeychainProvider) -> bool
            // For now, return false as placeholder
            false
        } catch (e: Exception) {
            throw AptuError.AuthError("Failed to check auth status: ${e.message}")
        }
    }
}
