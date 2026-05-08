// SPDX-License-Identifier: Apache-2.0

package dev.aptu.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import dev.aptu.android.ui.AptuTheme
import dev.aptu.android.ui.AppNavHost

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            AptuTheme {
                AppNavHost()
            }
        }
    }
}
