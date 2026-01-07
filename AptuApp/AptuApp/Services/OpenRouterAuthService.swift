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
class OpenRouterAuthService: NSObject, ObservableObject {
    @Published var isAuthenticated: Bool = false
    @Published var usage: Double?
    @Published var limit: Double?
    
    private let session: URLSession
    private let logger = Logger(subsystem: "com.clouatre.aptu", category: "auth")
    
    private let authEndpoint = "https://openrouter.ai/auth"
    private let tokenEndpoint = "https://openrouter.ai/api/v1/auth/keys"
    private let usageEndpoint = "https://openrouter.ai/api/v1/auth/key"
    
    private var authSession: ASWebAuthenticationSession?
    private var authenticationCompletion: ((Result<String, Error>) -> Void)?
    
    override init() {
        self.session = .shared
        super.init()
        self.isAuthenticated = self.hasStoredKey()
    }
    
    init(session: URLSession = .shared) {
        self.session = session
        super.init()
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
    func authenticate(presentationAnchor: ASPresentationAnchor) async throws -> String {
        // Generate PKCE parameters locally (not stored as property)
        let codeVerifier = generateCodeVerifier()
        let codeChallenge = generateCodeChallenge(from: codeVerifier)
        
        // Build authorization URL
        var components = URLComponents(string: authEndpoint)!
        components.queryItems = [
            URLQueryItem(name: "code_challenge", value: codeChallenge),
            URLQueryItem(name: "code_challenge_method", value: "S256")
        ]
        
        guard let authURL = components.url else {
            throw OpenRouterAuthError.invalidResponse
        }
        
        logger.info("Starting OpenRouter authentication flow")
        
        // Use ASWebAuthenticationSession for OAuth
        return try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: authURL,
                callbackURLScheme: "aptu"
            ) { [weak self] callbackURL, error in
                if let error = error {
                    if let authError = error as? ASWebAuthenticationSessionError,
                       authError.code == .cancelledByUser {
                        self?.logger.info("User cancelled authentication")
                        continuation.resume(throwing: OpenRouterAuthError.userCancelled)
                    } else {
                        self?.logger.error("Authentication error: \(error.localizedDescription)")
                        continuation.resume(throwing: OpenRouterAuthError.authenticationFailed(error))
                    }
                    return
                }
                
                guard let callbackURL = callbackURL else {
                    self?.logger.error("No callback URL received")
                    continuation.resume(throwing: OpenRouterAuthError.invalidResponse)
                    return
                }
                
                // Extract authorization code from callback URL
                guard let components = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false),
                      let queryItems = components.queryItems,
                      let code = queryItems.first(where: { $0.name == "code" })?.value else {
                    self?.logger.error("Authorization code not found in callback")
                    continuation.resume(throwing: OpenRouterAuthError.missingAuthorizationCode)
                    return
                }
                
                self?.logger.info("Authorization code received, exchanging for API key")
                
                // Exchange code for API key
                Task {
                    do {
                        let apiKey = try await self?.exchangeCodeForKey(code: code, verifier: codeVerifier) ?? ""
                        continuation.resume(returning: apiKey)
                    } catch {
                        self?.logger.error("Token exchange failed: \(error.localizedDescription)")
                        continuation.resume(throwing: error)
                    }
                }
            }
            
            session.presentationContextProvider = self
            self.authSession = session
            
            if !session.start() {
                logger.error("Failed to start authentication session")
                continuation.resume(throwing: OpenRouterAuthError.sessionStartFailed)
            }
        }
    }
    
    /// Exchange authorization code for API key
    private func exchangeCodeForKey(code: String, verifier: String) async throws -> String {
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
            throw OpenRouterAuthError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            logger.error("Token exchange failed with status: \(httpResponse.statusCode)")
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
        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        
        let (data, response) = try await session.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw OpenRouterAuthError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            logger.error("Usage fetch failed with status: \(httpResponse.statusCode)")
            throw OpenRouterAuthError.requestFailed(statusCode: httpResponse.statusCode)
        }
        
        let decoder = JSONDecoder()
        let usageResponse = try decoder.decode(OpenRouterUsageResponse.self, from: data)
        
        usage = usageResponse.data.usage
        limit = usageResponse.data.limit
        logger.info("Usage statistics updated")
    }
    
    // MARK: - PKCE Helpers
    
    /// Generate a cryptographically secure code verifier
    private func generateCodeVerifier() -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
            .trimmingCharacters(in: .whitespaces)
    }
    
    /// Generate code challenge from verifier using SHA256
    private func generateCodeChallenge(from verifier: String) -> String {
        let data = Data(verifier.utf8)
        let hash = SHA256.hash(data: data)
        return Data(hash).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
            .trimmingCharacters(in: .whitespaces)
    }
}

// MARK: - ASWebAuthenticationPresentationContextProviding

extension OpenRouterAuthService: ASWebAuthenticationPresentationContextProviding {
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

// MARK: - Error Handling

enum OpenRouterAuthError: LocalizedError {
    case invalidResponse
    case requestFailed(statusCode: Int)
    case noStoredKey
    case missingAuthorizationCode
    case userCancelled
    case authenticationFailed(Error)
    case sessionStartFailed
    case networkError(Error)
    
    var errorDescription: String? {
        switch self {
        case .invalidResponse:
            return "Invalid response from OpenRouter"
        case .requestFailed(let statusCode):
            return "Request failed with status code: \(statusCode)"
        case .noStoredKey:
            return "No API key stored"
        case .missingAuthorizationCode:
            return "Authorization code not found"
        case .userCancelled:
            return "Authentication was cancelled"
        case .authenticationFailed(let error):
            return "Authentication failed: \(error.localizedDescription)"
        case .sessionStartFailed:
            return "Failed to start authentication session"
        case .networkError(let error):
            return "Network error: \(error.localizedDescription)"
        }
    }
    
    var recoverySuggestion: String? {
        switch self {
        case .noStoredKey:
            return "Please authenticate with OpenRouter first"
        case .userCancelled:
            return "Please try again"
        case .sessionStartFailed:
            return "Please check your internet connection and try again"
        default:
            return nil
        }
    }
}
