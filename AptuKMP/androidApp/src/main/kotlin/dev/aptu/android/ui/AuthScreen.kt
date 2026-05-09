// SPDX-License-Identifier: Apache-2.0

package dev.aptu.android.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import dev.aptu.shared.viewmodels.AuthState
import dev.aptu.shared.viewmodels.AuthViewModel

@Composable
fun AuthScreen(
    viewModel: AuthViewModel,
    onAuthSuccess: () -> Unit,
) {
    val state = viewModel.state.collectAsState()

    // Key on AuthState.Success specifically so the effect only fires once on success,
    // not on every state transition.
    val isSuccess = state.value is AuthState.Success
    LaunchedEffect(isSuccess) {
        if (isSuccess) {
            onAuthSuccess()
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        when (val currentState = state.value) {
            AuthState.Idle -> {
                Text(
                    text = "Aptu",
                    style = MaterialTheme.typography.headlineLarge,
                )
                Spacer(modifier = Modifier.height(32.dp))
                Button(
                    onClick = { viewModel.startAuth() },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Sign in with GitHub")
                }
            }

            AuthState.RequestingCode -> {
                CircularProgressIndicator()
                Spacer(modifier = Modifier.height(16.dp))
                Text("Requesting device code...")
            }

            is AuthState.WaitingForAuth -> {
                val clipboardManager = LocalClipboardManager.current
                val uriHandler = LocalUriHandler.current
                Text(
                    text = "Device Code",
                    style = MaterialTheme.typography.labelMedium,
                )
                Spacer(modifier = Modifier.height(8.dp))
                TextField(
                    value = currentState.userCode,
                    onValueChange = {},
                    readOnly = true,
                    modifier = Modifier.fillMaxWidth(),
                    textStyle = MaterialTheme.typography.bodyMedium.copy(
                        fontFamily = FontFamily.Monospace,
                    ),
                )
                Spacer(modifier = Modifier.height(16.dp))
                Button(
                    onClick = {
                        clipboardManager.setText(AnnotatedString(currentState.userCode))
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Copy Code")
                }
                Spacer(modifier = Modifier.height(8.dp))
                OutlinedButton(
                    onClick = {
                        uriHandler.openUri(currentState.verificationUri)
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Open Browser")
                }
                Spacer(modifier = Modifier.height(16.dp))
                Text(
                    text = "Verification URL: ${currentState.verificationUri}",
                    style = MaterialTheme.typography.bodySmall,
                )
                Spacer(modifier = Modifier.height(16.dp))
                OutlinedButton(
                    onClick = { viewModel.cancel() },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Cancel")
                }
            }

            is AuthState.Polling -> {
                CircularProgressIndicator()
                Spacer(modifier = Modifier.height(16.dp))
                LinearProgressIndicator(
                    progress = { currentState.current.toFloat() / currentState.total.toFloat() },
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(modifier = Modifier.height(8.dp))
                Text("Waiting for authorization... ${currentState.current}/${currentState.total}")
            }

            AuthState.Success -> {
                Text("Authentication successful!")
            }

            is AuthState.Error -> {
                Text(
                    text = "Error: ${currentState.message}",
                    color = MaterialTheme.colorScheme.error,
                )
                Spacer(modifier = Modifier.height(16.dp))
                Button(
                    onClick = { viewModel.retry() },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Try Again")
                }
            }
        }
    }
}
