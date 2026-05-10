// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared.viewmodels

import dev.aptu.shared.AptuError
import dev.aptu.shared.AptuFfi
import dev.aptu.shared.AptuKeychain
import dev.aptu.shared.models.Issue
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

sealed class IssueState {
    data object Loading : IssueState()
    data class Success(val issues: List<Issue>) : IssueState()
    data class Error(val message: String) : IssueState()
}

class IssueViewModel {
    private val _state = MutableStateFlow<IssueState>(IssueState.Loading)
    val state: StateFlow<IssueState> = _state.asStateFlow()

    suspend fun load(keychain: AptuKeychain) {
        _state.value = IssueState.Loading
        try {
            val issues = AptuFfi.fetchIssues(keychain)
            _state.value = IssueState.Success(issues)
        } catch (e: AptuError.AuthError) {
            _state.value = IssueState.Error("Authentication required")
        } catch (e: AptuError.NetworkError) {
            _state.value = IssueState.Error("Network error: ${e.message}")
        } catch (e: AptuError) {
            _state.value = IssueState.Error(e.message ?: "Failed to load issues")
        }
    }

    suspend fun refresh(keychain: AptuKeychain) {
        load(keychain)
    }
}
