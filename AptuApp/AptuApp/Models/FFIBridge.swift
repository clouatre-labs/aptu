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
    
    private let keychainProvider: SwiftKeychainProvider
    
    private init() {
        self.keychainProvider = SwiftKeychainProvider()
    }
    
    /// Fetch issues for a repository using FFI binding
    func fetchIssues(owner: String, repo: String) async throws -> [Issue] {
        // Call actual UniFFI binding
        do {
            let ffiIssues = try fetchIssues(keychain: keychainProvider)
            
            // Map FfiIssueNode to Swift Issue models
            return ffiIssues.compactMap { ffiIssue -> Issue? in
                // Extract repository info from URL
                // URL format: https://github.com/owner/repo/issues/number
                let urlComponents = ffiIssue.url.components(separatedBy: "/")
                guard urlComponents.count >= 5 else { return nil }
                
                let repoOwner = urlComponents[3]
                let repoName = urlComponents[4]
                
                // Filter by requested owner/repo
                guard repoOwner == owner && repoName == repo else { return nil }
                
                // Map labels (FFI returns simple string array)
                let labels = ffiIssue.labels.enumerated().map { index, labelName in
                    IssueLabel(
                        id: "\(ffiIssue.number)-\(index)",
                        name: labelName,
                        color: "808080", // Default gray color
                        description: nil
                    )
                }
                
                return Issue(
                    id: "\(ffiIssue.number)",
                    number: Int(ffiIssue.number),
                    title: ffiIssue.title,
                    body: ffiIssue.body.isEmpty ? nil : ffiIssue.body,
                    author: "unknown", // FfiIssueNode doesn't include author
                    createdAt: ffiIssue.createdAt,
                    updatedAt: ffiIssue.updatedAt,
                    labels: labels,
                    repositoryName: repoName,
                    repositoryOwner: repoOwner,
                    url: ffiIssue.url
                )
            }
        } catch {
            throw FFIBridgeError.fetchFailed(error.localizedDescription)
        }
    }
    
    /// List curated repositories using FFI binding
    func listCuratedRepositories() async throws -> [Repository] {
        // Call actual UniFFI binding
        do {
            let ffiRepos = try listCuratedRepos()
            
            // Map FfiCuratedRepo to Swift Repository models
            return ffiRepos.map { ffiRepo in
                Repository(
                    id: "\(ffiRepo.owner)/\(ffiRepo.name)",
                    name: ffiRepo.name,
                    owner: ffiRepo.owner,
                    description: ffiRepo.description.isEmpty ? nil : ffiRepo.description
                )
            }
        } catch {
            throw FFIBridgeError.fetchFailed(error.localizedDescription)
        }
    }
}
