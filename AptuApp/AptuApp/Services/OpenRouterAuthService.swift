// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import Foundation
import CryptoKit
import AuthenticationServices
import os

// MARK: - Response Models

struct OpenRouterTokenResponse: Codable {
    let key: String
}

struct OpenRouterUsageResponse: Codable {
    let data: UsageData
    
    struct UsageData: Codable {
        let usage: Double
        let limit: Double?
    }
}

// MARK: - OpenRouter Auth Service

@MainActor
class OpenRouterAuthService: ObservableObject {
    @Published var isAuthenticated: Bool = false
    @Published var usage: Double?
    @Published var limit: Double?
    
    private let session: URLSession
    private let logger = Logger(subsystem: "com.aptu.openrouter", category: "auth")
    
    private let authEndpoint = "https://openrouter.ai/auth"
    private let tokenEndpoint = "https://openrouter.ai/api/v1/auth/keys"
    private let usageEndpoint = "https://openrouter.ai/api/v1/auth/key"
    
    init(session: URLSession = .shared) {
        self.session = session
        self.isAuthenticated = self.hasStoredKey()
    }
    
    // MARK: - Public Methods
    
    /// Check if API key is stored
    func hasStoredKey() -> Bool {
        return SwiftKeychain.shared.getToken(service: "aptu", account: "openrouter") != nil
    }
    
    /// Get stored API key
    func getStoredKey() throws -> String {
        guard let key = SwiftKeychain.shared.getToken(service: "aptu", account: "openrouter") else {
            throw OpenRouterAuthError.noStoredKey
        }
        return key
    }
    
    /// Store API key in keychain
    func storeKey(_ key: String) throws {
        try SwiftKeychain.shared.setToken(service: "aptu", account: "openrouter", token: key)
        isAuthenticated = true
    }
    
    /// Remove stored API key
    func removeKey() throws {
        try SwiftKeychain.shared.deleteToken(service: "aptu", account: "openrouter")
        isAuthenticated = false
        usage = nil
        limit = nil
    }
    
    /// Authenticate with OpenRouter using ASWebAuthenticationSession
    func authenticate(presentationContextProvider: ASWebAuthenticationPresentationContextProviding) async throws {
        let verifier = generateCodeVerifier()
        let challenge = generateCodeChallenge(from: verifier)
        
        var components = URLComponents(string: authEndpoint)!
        components.queryItems = [
            URLQueryItem(name: "callback_url", value: "aptu-openrouter://oauth"),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: "S256")
        ]
        
        guard let authURL = components.url else {
            logger.error("Failed to construct auth URL")
            throw OpenRouterAuthError.invalidResponse
        }
        
        logger.info("Starting OAuth flow with ASWebAuthenticationSession", privacy: .private)
        
        let session = ASWebAuthenticationSession(
            url: authURL,
            callbackURLScheme: "aptu-openrouter"
        ) { [weak self] result in
            Task {
                do {
                    switch result {
                    case .success(let callbackURL):
                        self?.logger.info("OAuth callback received")
                        if let code = self?.extractAuthCode(from: callbackURL) {
                            let key = try await self?.exchangeCodeForKey(code: code, verifier: verifier)
                            self?.logger.info("Successfully exchanged code for API key")
                        } else {
                            self?.logger.error("Failed to extract authorization code from callback URL")
                            throw OpenRouterAuthError.invalidResponse
                        }
                    case .failure(let error):
                        if (error as NSError).code == ASWebAuthenticationSessionError.cancelledLogin.rawValue {
                            self?.logger.info("User cancelled OAuth flow")
                        } else {
                            self?.logger.error("OAuth error: \(error.localizedDescription, privacy: .public)")
                        }
                        throw error
                    }
                } catch {
                    self?.logger.error("Error during OAuth exchange: \(error.localizedDescription, privacy: .public)")
                }
            }
        }
        
        session.presentationContextProvider = presentationContextProvider
        
        if !session.start() {
            logger.error("Failed to start ASWebAuthenticationSession")
            throw OpenRouterAuthError.invalidResponse
        }
    }
    
    /// Extract authorization code from callback URL
    private func extractAuthCode(from url: URL) -> String? {
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: true),
              let queryItems = components.queryItems else {
            return nil
        }
        return queryItems.first(where: { $0.name == "code" })?.value
    }
    
    /// Exchange authorization code for API key
    func exchangeCodeForKey(code: String, verifier: String) async throws -> String {
        var request = URLRequest(url: URL(string: tokenEndpoint)!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        
        let body: [String: String] = [
            "code": code,
            "code_verifier": verifier
        ]
        request.httpBody = try JSONEncoder().encode(body)
        
        let (data, response) = try await session.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            logger.error("Invalid response from token endpoint")
            throw OpenRouterAuthError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            logger.error("Token exchange failed with status code: \(httpResponse.statusCode)")
            throw OpenRouterAuthError.requestFailed(statusCode: httpResponse.statusCode)
        }
        
        let decoder = JSONDecoder()
        let tokenResponse = try decoder.decode(OpenRouterTokenResponse.self, from: data)
        
        // Store the key
        try storeKey(tokenResponse.key)
        logger.info("API key stored successfully")
        
        return tokenResponse.key
    }
    
    /// Fetch usage statistics
    func fetchUsage() async throws {
        let apiKey = try getStoredKey()
        
        var request = URLRequest(url: URL(string: usageEndpoint)!)
        request.httpMethod = "GET"
        request.setValue("Bearer \(apiKey, privacy: .private)", forHTTPHeaderField: "Authorization")
        
        logger.info("Fetching usage statistics")
        
        let (data, response) = try await session.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            logger.error("Invalid response from usage endpoint")
            throw OpenRouterAuthError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            logger.error("Usage fetch failed with status code: \(httpResponse.statusCode)")
            throw OpenRouterAuthError.requestFailed(statusCode: httpResponse.statusCode)
        }
        
        let decoder = JSONDecoder()
        let usageResponse = try decoder.decode(OpenRouterUsageResponse.self, from: data)
        
        usage = usageResponse.data.usage
        limit = usageResponse.data.limit
        logger.info("Usage statistics updated successfully")
    }
    
    // MARK: - PKCE Helpers
    
    /// Generate a cryptographically secure code verifier
    func generateCodeVerifier() -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
            .trimmingCharacters(in: .whitespaces)
    }
    
    /// Generate code challenge from verifier using SHA256
    func generateCodeChallenge(from verifier: String) -> String {
        let data = Data(verifier.utf8)
        let hash = SHA256.hash(data: data)
        return Data(hash).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
            .trimmingCharacters(in: .whitespaces)
    }
}

// MARK: - Error Handling

enum OpenRouterAuthError: LocalizedError {
    case invalidResponse
    case requestFailed(statusCode: Int)
    case noStoredKey
    case missingCodeVerifier
    case networkError(Error)
    
    var errorDescription: String? {
        switch self {
        case .invalidResponse:
            return "Invalid response from OpenRouter"
        case .requestFailed(let statusCode):
            return "Request failed with status code: \(statusCode)"
        case .noStoredKey:
            return "No API key stored"
        case .missingCodeVerifier:
            return "Code verifier not found"
        case .networkError(let error):
            return "Network error: \(error.localizedDescription)"
        }
    }
    
    var recoverySuggestion: String? {
        switch self {
        case .noStoredKey:
            return "Please authenticate with OpenRouter first"
        case .missingCodeVerifier:
            return "Please restart the authentication process"
        default:
            return nil
        }
    }
}
