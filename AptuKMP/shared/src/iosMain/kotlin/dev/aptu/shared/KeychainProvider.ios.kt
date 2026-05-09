// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

import com.liftric.kvault.KVault

actual class AptuKeychain {
    // No-arg constructor uses default service name derived from bundle ID.
    private val vault = KVault()

    actual fun getToken(service: String, account: String): String? {
        val key = "$service/$account"
        return vault.string(forKey = key)
    }

    actual fun setToken(service: String, account: String, token: String) {
        val key = "$service/$account"
        vault.set(stringValue = token, forKey = key)
    }

    actual fun deleteToken(service: String, account: String) {
        val key = "$service/$account"
        vault.deleteObject(forKey = key)
    }
}
