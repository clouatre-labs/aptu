// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import SwiftUI

@main
struct AptuApp: App {
    @State private var isAuthenticated: Bool = false
    
    init() {
        // Initialize Rust FFI bindings
        // UniFFI-generated bindings are available in the AptuFFI module
        initializeRustFFI()
        
        // Check if user is already authenticated
        checkAuthenticationStatus()
    }
    
    var body: some Scene {
        WindowGroup {
            if isAuthenticated {
                ContentView()
            } else {
                AuthenticationView()
            }
        }
    }
    
    /// Initialize Rust FFI bindings
    private func initializeRustFFI() {
        // UniFFI bindings are automatically initialized when imported
        // Any additional setup can be done here
        print("Rust FFI bindings initialized")
    }
    
    /// Check if user is already authenticated
    private func checkAuthenticationStatus() {
        // This would check the keychain for an existing token
        // For now, we default to not authenticated
        isAuthenticated = false
    }
}
