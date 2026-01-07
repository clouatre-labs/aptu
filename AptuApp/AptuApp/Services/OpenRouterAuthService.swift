// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import Foundation
import CryptoKit

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
    private var codeVerifier: String?
    
    init(session: URLSession = .shared) {
        self.session = session
        self.isAuthenticated = self.hasStoredKey()
    }
    
    // MARK: - Public Methods
    
    /// Check if API key is stored
    func hasStoredKey() -> Bool {
        return SwiftKeychain.shared.getToken(service: AuthConstants.keychainService, account: AuthConstants.openRouterAccount) != nil
    }
    
    /// Get stored API key
    func getStoredKey() throws -> String {
        guard let key = SwiftKeychain.shared.getToken(service: AuthConstants.keychainService, account: AuthConstants.openRouterAccount) else {
            throw OpenRouterAuthError.noStoredKey
        }
        return key
    }
    
    /// Store API key in keychain
    func storeKey(_ key: String) throws {
        try SwiftKeychain.shared.setToken(service: AuthConstants.keychainService, account: AuthConstants.openRouterAccount, token: key)
        isAuthenticated = true
    }
    
    /// Remove stored API key
    func removeKey() throws {
        try SwiftKeychain.shared.deleteToken(service: AuthConstants.keychainService, account: AuthConstants.openRouterAccount)
        isAuthenticated = false
        usage = nil
        limit = nil
    }
    
    /// Generate authorization URL with PKCE
    func generateAuthURL() -> URL {
        let verifier = generateCodeVerifier()
        self.codeVerifier = verifier
        
        let challenge = generateCodeChallenge(from: verifier)
        
        var components = URLComponents(string: AuthConstants.authEndpoint)!
        components.queryItems = [
            URLQueryItem(name: "callback_url", value: AuthConstants.redirectUri),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: AuthConstants.codeChallengeMethod)
        ]
        
        return components.url!
    }
    
    /// Exchange authorization code for API key with timeout
    func exchangeCodeForKey(code: String, timeout: TimeInterval = AuthConstants.defaultTimeout) async throws -> String {
        guard let verifier = codeVerifier else {
            throw OpenRouterAuthError.missingCodeVerifier
        }
        
        var request = URLRequest(url: URL(string: AuthConstants.tokenEndpoint)!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        
        let body: [String: String] = [
            "code": code,
            "code_verifier": verifier
        ]
        request.httpBody = try JSONEncoder().encode(body)
        
        let (data, response) = try await session.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw OpenRouterAuthError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            throw OpenRouterAuthError.requestFailed(statusCode: httpResponse.statusCode)
        }
        
        let decoder = JSONDecoder()
        let tokenResponse = try decoder.decode(OpenRouterTokenResponse.self, from: data)
        
        // Store the key
        try storeKey(tokenResponse.key)
        
        // Clear the verifier
        codeVerifier = nil
        
        return tokenResponse.key
    }
    
    /// Fetch usage statistics
    func fetchUsage() async throws {
        let apiKey = try getStoredKey()
        
        var request = URLRequest(url: URL(string: AuthConstants.usageEndpoint)!)
        request.httpMethod = "GET"
        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        
        let (data, response) = try await session.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw OpenRouterAuthError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            throw OpenRouterAuthError.requestFailed(statusCode: httpResponse.statusCode)
        }
        
        let decoder = JSONDecoder()
        let usageResponse = try decoder.decode(OpenRouterUsageResponse.self, from: data)
        
        usage = usageResponse.data.usage
        limit = usageResponse.data.limit
    }
    
    // MARK: - PKCE Helpers
    
    /// Generate a cryptographically secure code verifier using RFC 7636 base64URL encoding
    func generateCodeVerifier() -> String {
        var bytes = [UInt8](repeating: 0, count: AuthConstants.codeVerifierLength)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes).base64URLEncoded()
    }
    
    /// Generate code challenge from verifier using SHA256 and RFC 7636 base64URL encoding
    func generateCodeChallenge(from verifier: String) -> String {
        let data = Data(verifier.utf8)
        let hash = SHA256.hash(data: data)
        return Data(hash).base64URLEncoded()
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
