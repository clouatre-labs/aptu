// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import SwiftUI

struct OpenRouterAuthView: View {
    @StateObject private var authService = OpenRouterAuthService()
    @Environment(\.openURL) var openURL
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var showSuccess = false
    
    var body: some View {
        NavigationView {
            Form {
                Section {
                    if authService.isAuthenticated {
                        authenticatedContent
                    } else {
                        unauthenticatedContent
                    }
                }
                
                if authService.isAuthenticated {
                    Section("Usage") {
                        usageContent
                    }
                }
                
                if let errorMessage = errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundColor(.red)
                            .font(.caption)
                    }
                }
            }
            .navigationTitle("OpenRouter")
            .navigationBarTitleDisplayMode(.inline)
            .alert("Connected", isPresented: $showSuccess) {
                Button("OK", role: .cancel) { }
            } message: {
                Text("Successfully connected to OpenRouter")
            }
            .onReceive(NotificationCenter.default.publisher(for: NSNotification.Name("OpenRouterOAuthCallback"))) { notification in
                if let userInfo = notification.userInfo,
                   let code = userInfo["code"] as? String {
                    handleOAuthCallback(code: code)
                }
            }
        }
    }
    
    private var authenticatedContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundColor(.green)
                Text("Connected")
                    .fontWeight(.medium)
            }
            
            Button(role: .destructive) {
                disconnect()
            } label: {
                Text("Disconnect")
            }
        }
    }
    
    private var unauthenticatedContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: "xmark.circle.fill")
                    .foregroundColor(.gray)
                Text("Not Connected")
                    .fontWeight(.medium)
            }
            
            Button {
                connect()
            } label: {
                if isLoading {
                    HStack {
                        ProgressView()
                            .progressViewStyle(CircularProgressViewStyle())
                        Text("Connecting...")
                    }
                } else {
                    Text("Connect to OpenRouter")
                }
            }
            .disabled(isLoading)
        }
    }
    
    private var usageContent: some View {
        Group {
            if isLoading {
                HStack {
                    ProgressView()
                        .progressViewStyle(CircularProgressViewStyle())
                    Text("Loading usage...")
                }
            } else if let usage = authService.usage {
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Usage:")
                        Spacer()
                        Text(String(format: "$%.2f", usage))
                            .fontWeight(.medium)
                    }
                    
                    if let limit = authService.limit {
                        HStack {
                            Text("Limit:")
                            Spacer()
                            Text(String(format: "$%.2f", limit))
                                .fontWeight(.medium)
                        }
                        
                        ProgressView(value: usage, total: limit)
                            .progressViewStyle(LinearProgressViewStyle())
                    }
                }
            } else {
                Button {
                    Task {
                        await loadUsage()
                    }
                } label: {
                    Text("Load Usage")
                }
            }
        }
    }
    
    private func connect() {
        isLoading = true
        errorMessage = nil
        
        let authURL = authService.generateAuthURL()
        openURL(authURL)
        
        isLoading = false
    }
    
    private func disconnect() {
        do {
            try authService.removeKey()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }
    
    private func loadUsage() async {
        isLoading = true
        errorMessage = nil
        
        do {
            try await authService.fetchUsage()
        } catch {
            errorMessage = error.localizedDescription
        }
        
        isLoading = false
    }
    
    func handleOAuthCallback(code: String) {
        Task {
            isLoading = true
            errorMessage = nil
            
            do {
                _ = try await authService.exchangeCodeForKey(code: code)
                showSuccess = true
                try await authService.fetchUsage()
            } catch {
                errorMessage = error.localizedDescription
            }
            
            isLoading = false
        }
    }
}

struct OpenRouterAuthView_Previews: PreviewProvider {
    static var previews: some View {
        OpenRouterAuthView()
    }
}
