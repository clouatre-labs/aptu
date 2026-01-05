//
//  AptuApp.swift
//  AptuApp
//
//  Main SwiftUI app entry point with Rust FFI initialization.
//

import SwiftUI

@main
struct AptuApp: App {
    @State private var isAuthenticated: Bool = false
    
    init() {
        // Initialize Rust FFI bindings
        // UniFFI-generated bindings are available in the AptuFFI module
        initializeRustFFI()
        // Check authentication status on app launch
        checkAuthenticationStatus()
    }
    
    var body: some Scene {
        WindowGroup {
            if isAuthenticated {
                ContentView()
            } else {
                LoginView()
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
    /// This method checks if a valid GitHub token exists in the system keychain.
    /// If a token is found, the user is considered authenticated and will skip the login screen.
    /// If no token is found or keychain access fails, the user will be presented with the login screen.
    private func checkAuthenticationStatus() {
        do {
            // Attempt to retrieve the GitHub token from the keychain
            if let token = SwiftKeychain.shared.getToken(service: "aptu", account: "github"),
               !token.isEmpty {
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
