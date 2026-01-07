// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import Foundation

/// Authentication state for OpenRouter OAuth flow
enum AuthState: Equatable {
    /// Initial state, no authentication in progress
    case idle
    
    /// Authentication flow is in progress
    case authenticating
    
    /// Authentication succeeded with credentials
    case success(credentials: String)
    
    /// User cancelled the authentication flow
    case cancelled
    
    /// Authentication timed out
    case timeout
    
    /// Authentication failed with error message
    case error(String)
    
    /// Check if currently authenticating
    var isAuthenticating: Bool {
        if case .authenticating = self {
            return true
        }
        return false
    }
    
    /// Check if authentication succeeded
    var isSuccess: Bool {
        if case .success = self {
            return true
        }
        return false
    }
    
    /// Get error message if in error state
    var errorMessage: String? {
        if case .error(let message) = self {
            return message
        }
        return nil
    }
}
