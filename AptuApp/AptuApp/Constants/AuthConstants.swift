// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import Foundation

/// OAuth configuration constants for OpenRouter authentication
enum AuthConstants {
    /// OpenRouter OAuth client ID
    static let clientId = "aptu"
    
    /// OAuth callback URL scheme and host
    static let redirectScheme = "aptu"
    static let redirectHost = "oauth"
    static let redirectUri = "aptu://oauth"
    
    /// OpenRouter OAuth endpoints
    static let authEndpoint = "https://openrouter.ai/auth"
    static let tokenEndpoint = "https://openrouter.ai/api/v1/auth/keys"
    static let usageEndpoint = "https://openrouter.ai/api/v1/auth/key"
    
    /// PKCE configuration
    static let codeChallengeMethod = "S256"
    static let codeVerifierLength = 32
    static let defaultTimeout: TimeInterval = 30.0
    
    /// Keychain configuration
    static let keychainService = "aptu"
    static let openRouterAccount = "openrouter"
    static let githubAccount = "github"
}
