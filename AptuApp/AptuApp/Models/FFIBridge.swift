// SPDX-License-Identifier: Apache-2.0
//
// FFIBridge.swift
// Aptu
//
// Swift wrapper layer for Rust FFI bindings

import Foundation

// MARK: - Models

struct Repository: Identifiable, Hashable {
    let id: String
    let name: String
    let owner: String
    let description: String?
    
    var fullName: String {
        "\(owner)/\(name)"
    }
    
    var displayName: String {
        fullName
    }
}

struct Issue: Identifiable, Hashable {
    let id: String
    let number: Int
    let title: String
    let body: String?
    let author: String
    let createdAt: String
    let updatedAt: String
    let labels: [IssueLabel]
    let repositoryName: String
    let repositoryOwner: String
    let url: String
    
    var displayRepository: String {
        "\(repositoryOwner)/\(repositoryName)"
    }
    
    var bodyPreview: String {
        guard let body = body, !body.isEmpty else {
            return "No description"
        }
        let preview = body.prefix(100)
        return preview.count < body.count ? "\(preview)..." : String(preview)
    }
}

struct IssueLabel: Identifiable, Hashable {
    let id: String
    let name: String
    let color: String
    let description: String?
}

// MARK: - FFI Bridge

enum FFIBridgeError: Error {
    case fetchFailed(String)
    case invalidData
    case notImplemented
}

@MainActor
class FFIBridge {
    static let shared = FFIBridge()
    
    private init() {}
    
    /// Fetch issues for a repository using FFI binding
    func fetchIssues(owner: String, repo: String) async throws -> [Issue] {
        // TODO: Call actual FFI binding when available
        // For now, return mock data for UI development
        let repository = "\(owner)/\(repo)"
        return mockIssues(for: repository)
    }
    
    /// List curated repositories using FFI binding
    func listCuratedRepositories() async throws -> [Repository] {
        // TODO: Call actual FFI binding when available
        // For now, return mock data for UI development
        return mockRepositories()
    }
    
    // MARK: - Mock Data (temporary)
    
    private func mockRepositories() -> [Repository] {
        [
            Repository(
                id: "1",
                name: "aptu",
                owner: "clouatre-labs",
                description: "Gamified OSS issue triage with AI assistance"
            ),
            Repository(
                id: "2",
                name: "goose",
                owner: "block",
                description: "AI-powered development agent"
            ),
            Repository(
                id: "3",
                name: "swift",
                owner: "apple",
                description: "The Swift Programming Language"
            )
        ]
    }
    
    private func mockIssues(for repository: String) -> [Issue] {
        [
            Issue(
                id: "1",
                number: 100,
                title: "Add dark mode support",
                body: "Users have requested dark mode support for better accessibility and reduced eye strain.",
                author: "user123",
                createdAt: "2024-01-01T10:00:00Z",
                updatedAt: "2024-01-02T15:30:00Z",
                labels: [
                    IssueLabel(id: "1", name: "enhancement", color: "00FF00", description: "New feature"),
                    IssueLabel(id: "2", name: "good first issue", color: "90EE90", description: "Good for newcomers")
                ],
                repositoryName: repository.components(separatedBy: "/").last ?? "",
                repositoryOwner: repository.components(separatedBy: "/").first ?? "",
                url: "https://github.com/\(repository)/issues/100"
            ),
            Issue(
                id: "2",
                number: 101,
                title: "Fix crash on iOS 17",
                body: "App crashes when launched on iOS 17 devices.",
                author: "developer456",
                createdAt: "2024-01-03T09:00:00Z",
                updatedAt: "2024-01-03T12:00:00Z",
                labels: [
                    IssueLabel(id: "3", name: "bug", color: "FF0000", description: "Something isn't working")
                ],
                repositoryName: repository.components(separatedBy: "/").last ?? "",
                repositoryOwner: repository.components(separatedBy: "/").first ?? "",
                url: "https://github.com/\(repository)/issues/101"
            )
        ]
    }
}
