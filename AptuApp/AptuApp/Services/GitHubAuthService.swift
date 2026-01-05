// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import Foundation

// MARK: - Response Models

struct DeviceCodeResponse: Codable {
    let device_code: String
    let user_code: String
    let verification_uri: String
    let expires_in: Int
    let interval: Int
}

struct PollResponse: Codable {
    let access_token: String?
    let token_type: String?
    let scope: String?
    let error: String?
    let error_description: String?
    let error_uri: String?
}

// MARK: - GitHub Auth Service

class GitHubAuthService {
    private let clientId: String
    private let session: URLSession
    
    private let deviceCodeEndpoint = "https://github.com/login/device/code"
    private let accessTokenEndpoint = "https://github.com/login/oauth/access_token"
    
    private let minPollInterval: TimeInterval = 5
    private let maxPollInterval: TimeInterval = 60
    
    init(clientId: String, session: URLSession = .shared) {
        self.clientId = clientId
        self.session = session
    }
    
    // MARK: - Public Methods
    
    /// Request a device code from GitHub
    func requestDeviceCode() async throws -> DeviceCodeResponse {
        var request = URLRequest(url: URL(string: deviceCodeEndpoint)!)
        request.httpMethod = "POST"
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        
        let body = "client_id=\(clientId)&scope=repo"
        request.httpBody = body.data(using: .utf8)
        
        let (data, response) = try await session.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw GitHubAuthError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            throw GitHubAuthError.requestFailed(statusCode: httpResponse.statusCode)
        }
        
        let decoder = JSONDecoder()
        let deviceCodeResponse = try decoder.decode(DeviceCodeResponse.self, from: data)
        
        return deviceCodeResponse
    }
    
    /// Poll for access token with exponential backoff
    func pollForToken(
        deviceCode: String,
        maxAttempts: Int = 120,
        onProgress: @escaping (Int, Int) -> Void
    ) async throws -> String {
        var pollInterval = minPollInterval
        var attempt = 0
        
        while attempt < maxAttempts {
            attempt += 1
            onProgress(attempt, maxAttempts)
            
            // Wait before polling (except on first attempt)
            if attempt > 1 {
                try await Task.sleep(nanoseconds: UInt64(pollInterval * 1_000_000_000))
            }
            
            do {
                let token = try await pollOnce(deviceCode: deviceCode)
                return token
            } catch GitHubAuthError.authorizationPending {
                // Continue polling
                pollInterval = min(pollInterval * 1.5, maxPollInterval)
                continue
            } catch GitHubAuthError.slowDown {
                // Increase interval and retry
                pollInterval = min(pollInterval + 5, maxPollInterval)
                continue
            } catch GitHubAuthError.expiredToken {
                throw GitHubAuthError.expiredToken
            }
        }
        
        throw GitHubAuthError.pollTimeout
    }
    
    // MARK: - Private Methods
    
    private func pollOnce(deviceCode: String) async throws -> String {
        var request = URLRequest(url: URL(string: accessTokenEndpoint)!)
        request.httpMethod = "POST"
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        
        let body = "client_id=\(clientId)&device_code=\(deviceCode)&grant_type=urn:ietf:params:oauth:grant-type:device_code"
        request.httpBody = body.data(using: .utf8)
        
        let (data, response) = try await session.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw GitHubAuthError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 || httpResponse.statusCode == 400 else {
            throw GitHubAuthError.requestFailed(statusCode: httpResponse.statusCode)
        }
        
        let decoder = JSONDecoder()
        let pollResponse = try decoder.decode(PollResponse.self, from: data)
        
        // Check for errors first
        if let error = pollResponse.error {
            switch error {
            case "authorization_pending":
                throw GitHubAuthError.authorizationPending
            case "slow_down":
                throw GitHubAuthError.slowDown
            case "expired_token":
                throw GitHubAuthError.expiredToken
            case "access_denied":
                throw GitHubAuthError.accessDenied
            default:
                throw GitHubAuthError.unknownError(error)
            }
        }
        
        guard let accessToken = pollResponse.access_token else {
            throw GitHubAuthError.invalidResponse
        }
        
        return accessToken
    }
}

// MARK: - Error Handling

enum GitHubAuthError: LocalizedError {
    case invalidResponse
    case requestFailed(statusCode: Int)
    case authorizationPending
    case slowDown
    case expiredToken
    case accessDenied
    case pollTimeout
    case unknownError(String)
    case networkError(Error)
    
    var errorDescription: String? {
        switch self {
        case .invalidResponse:
            return "Invalid response from GitHub"
        case .requestFailed(let statusCode):
            return "Request failed with status code: \(statusCode)"
        case .authorizationPending:
            return "Waiting for authorization"
        case .slowDown:
            return "GitHub requested slower polling"
        case .expiredToken:
            return "Device code expired"
        case .accessDenied:
            return "Authorization was denied"
        case .pollTimeout:
            return "Polling timed out"
        case .unknownError(let error):
            return "Unknown error: \(error)"
        case .networkError(let error):
            return "Network error: \(error.localizedDescription)"
        }
    }
    
    var recoverySuggestion: String? {
        switch self {
        case .expiredToken:
            return "Please start the authentication process again"
        case .accessDenied:
            return "Please try again and approve the authorization request"
        case .pollTimeout:
            return "The authentication request took too long. Please try again"
        default:
            return nil
        }
    }
}
