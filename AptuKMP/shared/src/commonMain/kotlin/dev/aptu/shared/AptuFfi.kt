// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

import dev.aptu.shared.models.Issue
import dev.aptu.shared.models.Repo
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

sealed class AptuError(override val message: String) : Exception(message) {
    data class IOException(val detail: String) : AptuError(detail)
    data class AuthError(val detail: String) : AptuError(detail)
    data class NetworkError(val detail: String) : AptuError(detail)
}

object AptuFfi {
    // Dispatchers.Default is used here because Dispatchers.IO is JVM-only and
    // not available in commonMain when iOS targets are included.  The actual FFI
    // calls are blocking stubs until Gobley generates the bindings.
    suspend fun listCuratedRepos(): List<Repo> = withContext(Dispatchers.Default) {
        try {
            // TODO: Call UniFFI-generated listCuratedRepos() once Gobley generates bindings
            emptyList()
        } catch (e: Exception) {
            throw AptuError.IOException("Failed to list curated repos: ${e.message}")
        }
    }

    suspend fun fetchIssues(keychain: AptuKeychain): List<Issue> = withContext(Dispatchers.Default) {
        try {
            // TODO: Call UniFFI-generated fetchIssues(keychain) once Gobley generates bindings
            emptyList()
        } catch (e: Exception) {
            throw AptuError.NetworkError("Failed to fetch issues: ${e.message}")
        }
    }

    suspend fun checkAuthStatus(keychain: AptuKeychain): Boolean = withContext(Dispatchers.Default) {
        try {
            // TODO: Call UniFFI-generated checkAuthStatus(keychain) once Gobley generates bindings
            false
        } catch (e: Exception) {
            throw AptuError.AuthError("Failed to check auth status: ${e.message}")
        }
    }
}
