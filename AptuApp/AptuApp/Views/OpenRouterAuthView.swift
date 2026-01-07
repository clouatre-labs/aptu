// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import SwiftUI
import AuthenticationServices

struct OpenRouterAuthView: View, ASWebAuthenticationPresentationContextProviding {
    @StateObject private var authService = OpenRouterAuthService()
    @State private var authState: AuthState = .idle
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var showSuccess = false
    @State private var authSession: ASWebAuthenticationSession?
    
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
            .alert("Authentication Timeout", isPresented: .constant(authState == .timeout)) {
                Button("OK", role: .cancel) {
                    authState = .idle
                }
            } message: {
                Text("Authentication took too long. Please try again.")
            }
            .alert("Authentication Cancelled", isPresented: .constant(authState == .cancelled)) {
                Button("OK", role: .cancel) {
                    authState = .idle
                }
            } message: {
                Text("You cancelled the authentication flow.")
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
                startAuthentication()
            } label: {
                if authState.isAuthenticating {
                    HStack {
                        ProgressView()
                            .progressViewStyle(CircularProgressViewStyle())
                        Text("Connecting...")
                    }
                } else {
                    Text("Connect to OpenRouter")
                }
            }
            .disabled(authState.isAuthenticating)
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
    
    private func startAuthentication() {
        authState = .authenticating
        errorMessage = nil
        
        let authURL = authService.generateAuthURL()
        
        authSession = ASWebAuthenticationSession(
            url: authURL,
            callbackURLScheme: AuthConstants.redirectScheme
        ) { callbackURL, error in
            handleAuthenticationResult(callbackURL: callbackURL, error: error)
        }
        
        authSession?.presentationContextProvider = self
        
        // Set timeout for authentication
        Task {
            try? await Task.sleep(nanoseconds: UInt64(AuthConstants.defaultTimeout * 1_000_000_000))
            if authState.isAuthenticating {
                authState = .timeout
                authSession?.cancel()
            }
        }
        
        authSession?.start()
    }
    
    private func handleAuthenticationResult(callbackURL: URL?, error: Error?) {
        if let error = error as? ASWebAuthenticationSessionError {
            if error.code == .cancelledByUser {
                authState = .cancelled
            } else {
                authState = .error("Authentication failed: \(error.localizedDescription)")
                errorMessage = error.localizedDescription
            }
            return
        }
        
        if let error = error {
            authState = .error("Authentication failed: \(error.localizedDescription)")
            errorMessage = error.localizedDescription
            return
        }
        
        guard let callbackURL = callbackURL,
              let components = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false),
              let queryItems = components.queryItems,
              let code = queryItems.first(where: { $0.name == "code" })?.value else {
            authState = .error("Missing authorization code")
            errorMessage = "Missing authorization code in callback"
            return
        }
        
        handleOAuthCallback(code: code)
    }
    
    private func disconnect() {
        do {
            try authService.removeKey()
            errorMessage = nil
            authState = .idle
        } catch {
            errorMessage = error.localizedDescription
            authState = .error(error.localizedDescription)
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
    
    private func handleOAuthCallback(code: String) {
        Task {
            authState = .authenticating
            errorMessage = nil
            
            do {
                _ = try await authService.exchangeCodeForKey(code: code)
                authState = .success(credentials: code)
                showSuccess = true
                try await authService.fetchUsage()
                authState = .idle
            } catch {
                errorMessage = error.localizedDescription
                authState = .error(error.localizedDescription)
            }
        }
    }
    
    // MARK: - ASWebAuthenticationPresentationContextProviding
    
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        guard let window = UIApplication.shared.connectedScenes
            .compactMap({ $0 as? UIWindowScene })
            .flatMap({ $0.windows })
            .first(where: { $0.isKeyWindow }) else {
            return ASPresentationAnchor()
        }
        return window
    }
}

struct OpenRouterAuthView_Previews: PreviewProvider {
    static var previews: some View {
        OpenRouterAuthView()
    }
}
