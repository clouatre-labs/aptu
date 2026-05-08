// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

expect class AptuKeychain() : IAptuKeychain

fun aptuKeychain(): AptuKeychain = AptuKeychain()
