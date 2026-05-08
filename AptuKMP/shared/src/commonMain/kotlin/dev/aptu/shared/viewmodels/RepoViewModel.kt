// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared.viewmodels

import dev.aptu.shared.AptuFfi
import dev.aptu.shared.models.Repo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

sealed class RepoState {
    data object Loading : RepoState()
    data class Success(val repos: List<Repo>) : RepoState()
    data class Error(val message: String) : RepoState()
}

class RepoViewModel {
    private val _state = MutableStateFlow<RepoState>(RepoState.Loading)
    val state: StateFlow<RepoState> = _state.asStateFlow()

    private var allRepos: List<Repo> = emptyList()

    suspend fun load() {
        _state.value = RepoState.Loading
        try {
            allRepos = AptuFfi.listCuratedRepos()
            _state.value = RepoState.Success(allRepos)
        } catch (e: Exception) {
            _state.value = RepoState.Error(e.message ?: "Unknown error")
        }
    }

    fun filter(query: String) {
        val filtered = if (query.isBlank()) {
            allRepos
        } else {
            allRepos.filter { repo ->
                repo.name.contains(query, ignoreCase = true) ||
                    repo.owner.contains(query, ignoreCase = true) ||
                    repo.description.contains(query, ignoreCase = true)
            }
        }
        _state.value = RepoState.Success(filtered)
    }
}
