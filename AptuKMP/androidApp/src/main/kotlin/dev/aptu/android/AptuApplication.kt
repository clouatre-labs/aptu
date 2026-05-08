// SPDX-License-Identifier: Apache-2.0

package dev.aptu.android

import android.app.Application
import com.github.nickolay.kvault.KVault

class AptuApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        // KVault requires an Android Context for EncryptedSharedPreferences initialization.
        // Initialize once here so AptuKeychain can reference the singleton safely from any thread.
        KVault.init(this)
    }
}
