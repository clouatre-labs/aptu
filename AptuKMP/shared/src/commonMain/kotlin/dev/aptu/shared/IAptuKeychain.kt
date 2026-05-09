// SPDX-FileCopyrightText: 2026 Hugues Clouâtre <hugues@clouatre.dev>
// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

interface IAptuKeychain {
    fun getToken(service: String, account: String): String?
    fun setToken(service: String, account: String, token: String)
    fun deleteToken(service: String, account: String)
}
