//
//  AptuApp.swift
//  AptuApp
//
//  Main SwiftUI app entry point with Rust FFI initialization.
//

import SwiftUI

@main
struct AptuApp: App {
    init() {
        // Initialize Rust FFI bindings
        // UniFFI-generated bindings are available in the AptuFFI module
        initializeRustFFI()
    }
    
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
    
    /// Initialize Rust FFI bindings
    private func initializeRustFFI() {
        // UniFFI bindings are automatically initialized when imported
        // Any additional setup can be done here
        print("Rust FFI bindings initialized")
    }
}
