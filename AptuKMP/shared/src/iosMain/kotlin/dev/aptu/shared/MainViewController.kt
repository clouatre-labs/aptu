// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.window.ComposeUIViewController

fun MainViewController() = ComposeUIViewController {
    AppContent()
}

@Composable
fun AppContent() {
    MaterialTheme {
        Text("AptuKMP iOS")
    }
}
