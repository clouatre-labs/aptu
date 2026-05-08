// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class FakeAptuKeychain : AptuKeychain() {
    private val storage = mutableMapOf<String, String>()

    override fun getToken(service: String, account: String): String? {
        val key = "$service/$account"
        return storage[key]
    }

    override fun setToken(service: String, account: String, token: String) {
        val key = "$service/$account"
        storage[key] = token
    }

    override fun deleteToken(service: String, account: String) {
        val key = "$service/$account"
        storage.remove(key)
    }
}

class KeychainProviderTest {
    @Test
    fun testStoreAndRetrieveToken() {
        val keychain = FakeAptuKeychain()
        val service = "github"
        val account = "user@example.com"
        val token = "ghp_1234567890abcdef"

        keychain.setToken(service, account, token)
        val retrieved = keychain.getToken(service, account)

        assertEquals(token, retrieved)
    }

    @Test
    fun testDeleteToken() {
        val keychain = FakeAptuKeychain()
        val service = "github"
        val account = "user@example.com"
        val token = "ghp_1234567890abcdef"

        keychain.setToken(service, account, token)
        keychain.deleteToken(service, account)
        val retrieved = keychain.getToken(service, account)

        assertNull(retrieved)
    }
}
