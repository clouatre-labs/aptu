// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

import android.content.Context
import android.content.SharedPreferences

// AptuKeychain.init(context) must be called in Application.onCreate() before
// any AptuKeychain instance is created.  SharedPreferences is used for the
// scaffold; encrypted storage (EncryptedSharedPreferences or KVault) is a
// follow-up once the OAuth flow is wired in.
class AptuKeychain : IAptuKeychain {
    companion object {
        private lateinit var prefs: SharedPreferences

        fun init(context: Context) {
            prefs = context.applicationContext
                .getSharedPreferences("aptu_keychain", Context.MODE_PRIVATE)
        }
    }

    override fun getToken(service: String, account: String): String? {
        val key = "$service/$account"
        return prefs.getString(key, null)
    }

    override fun setToken(service: String, account: String, token: String) {
        val key = "$service/$account"
        prefs.edit().putString(key, token).apply()
    }

    override fun deleteToken(service: String, account: String) {
        val key = "$service/$account"
        prefs.edit().remove(key).apply()
    }
}

fun aptuKeychain(): AptuKeychain = AptuKeychain()
