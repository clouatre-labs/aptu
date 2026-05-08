// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

expect class AptuKeychain() {
    fun getToken(service: String, account: String): String?
    fun setToken(service: String, account: String, token: String)
    fun deleteToken(service: String, account: String)
}

fun aptuKeychain(): AptuKeychain = AptuKeychain()
