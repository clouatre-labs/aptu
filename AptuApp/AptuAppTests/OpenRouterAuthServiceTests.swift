// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import XCTest
@testable import AptuApp

final class OpenRouterAuthServiceTests: XCTestCase {
    var authService: OpenRouterAuthService!
    
    override func setUp() {
        super.setUp()
        authService = OpenRouterAuthService()
    }
    
    override func tearDown() {
        authService = nil
        super.tearDown()
    }
    
    // MARK: - PKCE Tests
    
    func testGenerateCodeVerifier() {
        // Arrange & Act
        let verifier = authService.generateCodeVerifier()
        
        // Assert
        XCTAssertFalse(verifier.isEmpty, "Code verifier should not be empty")
        XCTAssertGreaterThan(verifier.count, 40, "Code verifier should be sufficiently long")
        XCTAssertFalse(verifier.contains("+"), "Code verifier should not contain +")
        XCTAssertFalse(verifier.contains("/"), "Code verifier should not contain /")
        XCTAssertFalse(verifier.contains("="), "Code verifier should not contain =")
    }
    
    func testGenerateCodeChallenge() {
        // Arrange
        let verifier = "test_verifier_12345"
        
        // Act
        let challenge = authService.generateCodeChallenge(from: verifier)
        
        // Assert
        XCTAssertFalse(challenge.isEmpty, "Code challenge should not be empty")
        XCTAssertFalse(challenge.contains("+"), "Code challenge should not contain +")
        XCTAssertFalse(challenge.contains("/"), "Code challenge should not contain /")
        XCTAssertFalse(challenge.contains("="), "Code challenge should not contain =")
    }
    
    func testGenerateCodeChallengeIsConsistent() {
        // Arrange
        let verifier = "test_verifier_12345"
        
        // Act
        let challenge1 = authService.generateCodeChallenge(from: verifier)
        let challenge2 = authService.generateCodeChallenge(from: verifier)
        
        // Assert
        XCTAssertEqual(challenge1, challenge2, "Same verifier should produce same challenge")
    }
    
    func testGenerateCodeVerifierIsUnique() {
        // Arrange & Act
        let verifier1 = authService.generateCodeVerifier()
        let verifier2 = authService.generateCodeVerifier()
        
        // Assert
        XCTAssertNotEqual(verifier1, verifier2, "Each verifier should be unique")
    }
    
    // MARK: - Auth URL Tests
    
    func testGenerateAuthURL() {
        // Arrange & Act
        let authURL = authService.generateAuthURL()
        
        // Assert
        XCTAssertEqual(authURL.scheme, "https", "Auth URL should use HTTPS")
        XCTAssertEqual(authURL.host, "openrouter.ai", "Auth URL should point to OpenRouter")
        XCTAssertEqual(authURL.path, "/auth", "Auth URL should use /auth path")
        
        let components = URLComponents(url: authURL, resolvingAgainstBaseURL: false)
        let queryItems = components?.queryItems ?? []
        
        XCTAssertTrue(queryItems.contains(where: { $0.name == "callback_url" && $0.value == "aptu://oauth" }), "Should include callback URL")
        XCTAssertTrue(queryItems.contains(where: { $0.name == "code_challenge" }), "Should include code challenge")
        XCTAssertTrue(queryItems.contains(where: { $0.name == "code_challenge_method" && $0.value == "S256" }), "Should use S256 method")
    }
    
    // MARK: - Base64URL Encoding Tests
    
    func testBase64URLEncoding() {
        // Arrange
        let testData = Data("test_string".utf8)
        
        // Act
        let encoded = testData.base64URLEncoded()
        
        // Assert
        XCTAssertFalse(encoded.contains("+"), "Should not contain +")
        XCTAssertFalse(encoded.contains("/"), "Should not contain /")
        XCTAssertFalse(encoded.contains("="), "Should not contain padding")
    }
    
    func testBase64URLDecoding() {
        // Arrange
        let originalData = Data("test_string".utf8)
        let encoded = originalData.base64URLEncoded()
        
        // Act
        let decoded = Data.base64URLDecoded(encoded)
        
        // Assert
        XCTAssertEqual(decoded, originalData, "Decoded data should match original")
    }
    
    // MARK: - Token Exchange Tests
    
    func testExchangeCodeForKeyAsync() async {
        // Arrange
        let mockSession = MockURLSession()
        let service = OpenRouterAuthService(session: mockSession)
        
        // Generate auth URL to set code verifier
        _ = service.generateAuthURL()
        
        // Act & Assert
        do {
            let key = try await service.exchangeCodeForKey(code: "test_code")
            XCTAssertEqual(key, "test_api_key", "Should return the API key from response")
        } catch {
            XCTFail("Should not throw error: \(error)")
        }
    }
    
    func testExchangeCodeForKeyMissingVerifier() async {
        // Arrange
        let service = OpenRouterAuthService()
        
        // Act & Assert
        do {
            _ = try await service.exchangeCodeForKey(code: "test_code")
            XCTFail("Should throw missingCodeVerifier error")
        } catch OpenRouterAuthError.missingCodeVerifier {
            // Expected
        } catch {
            XCTFail("Should throw missingCodeVerifier, not \(error)")
        }
    }
}

// MARK: - Mock URLSession

class MockURLSession: URLSession {
    override func data(for request: URLRequest) async throws -> (Data, URLResponse) {
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: 200,
            httpVersion: nil,
            headerFields: nil
        )!
        
        let responseData = try JSONEncoder().encode(OpenRouterTokenResponse(key: "test_api_key"))
        return (responseData, response)
    }
}
