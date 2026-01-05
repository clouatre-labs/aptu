// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import SwiftUI

struct AuthenticationView: View {
    @StateObject private var viewModel = AuthenticationViewModel()
    @Environment(\.openURL) var openURL
    
    var body: some View {
        ZStack {
            Color(.systemBackground)
                .ignoresSafeArea()
            
            VStack(spacing: 24) {
                // Header
                VStack(spacing: 8) {
                    Image(systemName: "lock.shield")
                        .font(.system(size: 48))
                        .foregroundColor(.blue)
                    
                    Text("Authenticate with GitHub")
                        .font(.title2)
                        .fontWeight(.semibold)
                    
                    Text("Sign in to access GitHub repositories")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
                .padding(.top, 32)
                
                Spacer()
                
                // Content based on state
                switch viewModel.state {
                case .idle:
                    idleContent
                case .requestingCode:
                    loadingContent(message: "Requesting device code...")
                case .waitingForAuth(let response):
                    authorizationContent(response: response)
                case .polling(let progress):
                    pollingContent(progress: progress)
                case .success:
                    successContent
                case .error(let error):
                    errorContent(error: error)
                }
                
                Spacer()
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 16)
        }
    }
    
    // MARK: - Content Views
    
    private var idleContent: some View {
        VStack(spacing: 16) {
            Button(action: {
                Task {
                    await viewModel.startAuthentication()
                }
            }) {
                HStack {
                    Image(systemName: "arrow.right.circle.fill")
                    Text("Start Authentication")
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 12)
                .background(Color.blue)
                .foregroundColor(.white)
                .cornerRadius(8)
            }
        }
    }
    
    private func loadingContent(message: String) -> some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.5)
            
            Text(message)
                .font(.subheadline)
                .foregroundColor(.secondary)
        }
    }
    
    private func authorizationContent(response: DeviceCodeResponse) -> some View {
        VStack(spacing: 20) {
            VStack(spacing: 12) {
                Text("Device Code")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                
                HStack {
                    Text(response.user_code)
                        .font(.system(.title3, design: .monospaced))
                        .fontWeight(.semibold)
                        .tracking(2)
                    
                    Spacer()
                    
                    Button(action: {
                        UIPasteboard.general.string = response.user_code
                    }) {
                        Image(systemName: "doc.on.doc")
                            .foregroundColor(.blue)
                    }
                }
                .padding(.vertical, 12)
                .padding(.horizontal, 12)
                .background(Color(.systemGray6))
                .cornerRadius(8)
            }
            
            VStack(spacing: 12) {
                Text("Verification URL")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                
                Button(action: {
                    if let url = URL(string: response.verification_uri) {
                        openURL(url)
                    }
                }) {
                    HStack {
                        Image(systemName: "safari")
                        Text(response.verification_uri)
                            .lineLimit(1)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .padding(.horizontal, 12)
                    .background(Color.blue.opacity(0.1))
                    .foregroundColor(.blue)
                    .cornerRadius(8)
                }
            }
            
            VStack(spacing: 12) {
                Text("Instructions")
                    .font(.caption)
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity, alignment: .leading)
                
                VStack(alignment: .leading, spacing: 8) {
                    HStack(alignment: .top, spacing: 8) {
                        Text("1")
                            .font(.caption)
                            .fontWeight(.semibold)
                            .foregroundColor(.white)
                            .frame(width: 24, height: 24)
                            .background(Color.blue)
                            .cornerRadius(12)
                        
                        Text("Open the verification URL in your browser")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    
                    HStack(alignment: .top, spacing: 8) {
                        Text("2")
                            .font(.caption)
                            .fontWeight(.semibold)
                            .foregroundColor(.white)
                            .frame(width: 24, height: 24)
                            .background(Color.blue)
                            .cornerRadius(12)
                        
                        Text("Enter the device code: \(response.user_code)")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    
                    HStack(alignment: .top, spacing: 8) {
                        Text("3")
                            .font(.caption)
                            .fontWeight(.semibold)
                            .foregroundColor(.white)
                            .frame(width: 24, height: 24)
                            .background(Color.blue)
                            .cornerRadius(12)
                        
                        Text("Approve the authorization request")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
                .padding(.vertical, 12)
                .padding(.horizontal, 12)
                .background(Color(.systemGray6))
                .cornerRadius(8)
            }
            
            Button(role: .cancel, action: {
                viewModel.cancel()
            }) {
                Text("Cancel")
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .foregroundColor(.red)
            }
        }
    }
    
    private func pollingContent(progress: (current: Int, total: Int)) -> some View {
        VStack(spacing: 16) {
            ProgressView(value: Double(progress.current), total: Double(progress.total))
                .tint(.blue)
            
            VStack(spacing: 8) {
                Text("Waiting for Authorization")
                    .font(.subheadline)
                    .fontWeight(.semibold)
                
                Text("Checking for approval... (Attempt \(progress.current)/\(progress.total))")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Button(role: .cancel, action: {
                viewModel.cancel()
            }) {
                Text("Cancel")
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .foregroundColor(.red)
            }
        }
    }
    
    private var successContent: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 48))
                .foregroundColor(.green)
            
            VStack(spacing: 8) {
                Text("Authentication Successful")
                    .font(.subheadline)
                    .fontWeight(.semibold)
                
                Text("Your GitHub token has been securely stored")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
    }
    
    private func errorContent(error: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.circle.fill")
                .font(.system(size: 48))
                .foregroundColor(.red)
            
            VStack(spacing: 8) {
                Text("Authentication Failed")
                    .font(.subheadline)
                    .fontWeight(.semibold)
                
                Text(error)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
            
            Button(action: {
                viewModel.reset()
            }) {
                Text("Try Again")
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .background(Color.blue)
                    .foregroundColor(.white)
                    .cornerRadius(8)
            }
        }
    }
}

// MARK: - View Model

@MainActor
class AuthenticationViewModel: ObservableObject {
    @Published var state: AuthenticationState = .idle
    
    private let authService = GitHubAuthService(clientId: "YOUR_GITHUB_CLIENT_ID")
    private var pollTask: Task<Void, Never>?
    
    func startAuthentication() async {
        state = .requestingCode
        
        do {
            let response = try await authService.requestDeviceCode()
            state = .waitingForAuth(response)
            
            // Start polling for token
            await pollForToken(deviceCode: response.device_code)
        } catch {
            state = .error(error.localizedDescription)
        }
    }
    
    private func pollForToken(deviceCode: String) async {
        state = .polling((current: 0, total: 120))
        
        do {
            let token = try await authService.pollForToken(
                deviceCode: deviceCode,
                onProgress: { current, total in
                    Task { @MainActor in
                        self.state = .polling((current: current, total: total))
                    }
                }
            )
            
            // Store token in Keychain via FFI
            try await storeToken(token)
            state = .success
        } catch {
            state = .error(error.localizedDescription)
        }
    }
    
    private func storeToken(_ token: String) async throws {
        // Call Rust FFI to store token in Keychain
        // This will be implemented via uniffi bindings
        // For now, this is a placeholder
        try await Task.sleep(nanoseconds: 500_000_000) // Simulate async work
    }
    
    func cancel() {
        pollTask?.cancel()
        state = .idle
    }
    
    func reset() {
        state = .idle
    }
}

// MARK: - State

enum AuthenticationState {
    case idle
    case requestingCode
    case waitingForAuth(DeviceCodeResponse)
    case polling((current: Int, total: Int))
    case success
    case error(String)
}

#Preview {
    AuthenticationView()
}
