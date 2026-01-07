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
}
