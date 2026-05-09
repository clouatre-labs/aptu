// SPDX-License-Identifier: Apache-2.0

package dev.aptu.android

import android.app.Application
import dev.aptu.shared.AptuKeychain

class AptuApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        // Initialize the AptuKeychain SharedPreferences backing store before
        // any AptuKeychain instance is created.
        AptuKeychain.init(this)
    }
}
