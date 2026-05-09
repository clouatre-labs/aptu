// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

// expect class carries no supertype declaration: the KMP commonMain metadata
// compiler would require the expect class itself to satisfy all interface
// members, which it cannot (no body).  Each actual declares : IAptuKeychain
// independently, which is valid KMP.
expect class AptuKeychain() {
    fun getToken(service: String, account: String): String?
    fun setToken(service: String, account: String, token: String)
    fun deleteToken(service: String, account: String)
}

fun aptuKeychain(): AptuKeychain = AptuKeychain()
