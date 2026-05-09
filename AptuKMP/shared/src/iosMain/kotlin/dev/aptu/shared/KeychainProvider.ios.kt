// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

import com.liftric.kvault.KVault

actual class AptuKeychain : IAptuKeychain {
    private val vault = KVault()

    actual fun getToken(service: String, account: String): String? {
        val key = "$service/$account"
        return try {
            vault.get(key)
        } catch (e: Exception) {
            null
        }
    }

    actual fun setToken(service: String, account: String, token: String) {
        val key = "$service/$account"
        vault.set(key, token)
    }

    actual fun deleteToken(service: String, account: String) {
        val key = "$service/$account"
        try {
            vault.remove(key)
        } catch (e: Exception) {
            // Ignore if key does not exist
        }
    }
}
