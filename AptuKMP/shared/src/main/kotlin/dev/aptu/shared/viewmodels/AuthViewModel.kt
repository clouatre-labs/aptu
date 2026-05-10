// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared.viewmodels

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

sealed class AuthState {
    data object Idle : AuthState()
    data object RequestingCode : AuthState()
    data class WaitingForAuth(
        val userCode: String,
        val verificationUri: String,
    ) : AuthState()
    data class Polling(
        val current: Int,
        val total: Int,
    ) : AuthState()
    data object Success : AuthState()
    data class Error(val message: String) : AuthState()
}

class AuthViewModel {
    private val _state = MutableStateFlow<AuthState>(AuthState.Idle)
    val state: StateFlow<AuthState> = _state.asStateFlow()

    fun startAuth() {
        _state.value = AuthState.RequestingCode
        // TODO: Implement GitHub OAuth device flow
        // 1. POST to https://github.com/login/device/code with client_id
        // 2. Extract user_code, device_code, verification_uri
        // 3. Transition to WaitingForAuth
        // 4. Poll https://github.com/login/oauth/access_token with device_code
        // 5. On success, store token in keychain and transition to Success
        // 6. On error, transition to Error
        _state.value = AuthState.Error("Device flow not yet implemented")
    }

    fun cancel() {
        _state.value = AuthState.Idle
    }

    fun reset() {
        _state.value = AuthState.Idle
    }

    // Retry re-initiates the OAuth flow from a terminal error state.
    fun retry() {
        _state.value = AuthState.Idle
        startAuth()
    }
}
