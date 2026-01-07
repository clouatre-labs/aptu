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
    }
    
    var body: some Scene {
        WindowGroup {
            Group {
                if isAuthenticated {
                    ContentView()
                } else {
                    AuthenticationView()
                }
            }
            .task {
                // Check authentication status asynchronously on app launch
                // This ensures @State is fully initialized and main thread remains responsive
                await checkAuthenticationStatus()
            }
        }
    }
    
    /// Initialize Rust FFI bindings
    private func initializeRustFFI() {
        // UniFFI bindings are automatically initialized when imported
        // Any additional setup can be done here
        print("Rust FFI bindings initialized")
    }
    
    /// Check authentication status by verifying GitHub token in keychain
    ///
    /// This asynchronous method checks if a valid GitHub token exists in the system keychain.
    /// If a token is found, the user is considered authenticated and will skip the login screen.
    /// If no token is found or keychain access fails, the user will be presented with the login screen.
    /// 
    /// By using async/await, this operation doesn't block the main thread during app launch,
    /// ensuring responsive UI and preventing watchdog timeouts.
    private func checkAuthenticationStatus() async {
        do {
            // Attempt to retrieve the GitHub token from the keychain
            // Performed on a background thread to avoid blocking the main thread
            let token = try await Task.detached(priority: .userInitiated) {
                SwiftKeychain.shared.getToken(service: "aptu", account: "github")
            }.value
            
            if let token = token, !token.isEmpty {
                // Token exists and is non-empty, user is authenticated
                isAuthenticated = true
                print("Authentication check: Token found, user is authenticated")
            } else {
                // No token found, user needs to authenticate
                isAuthenticated = false
                print("Authentication check: No token found, user needs to login")
            }
        } catch {
            // Gracefully handle keychain access failures
            isAuthenticated = false
            print("Authentication check failed: \(error.localizedDescription)")
        }
    }
}
