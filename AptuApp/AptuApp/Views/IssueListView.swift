//
//  IssueListView.swift
//  AptuApp
//
//  SPDX-License-Identifier: Apache-2.0
//
//  Main issue list view with fetch_issues FFI binding integration.
//

import SwiftUI

struct IssueListView: View {
    let repository: Repository
    
    @State private var issues: [Issue] = []
    @State private var isLoading: Bool = false
    @State private var errorMessage: String?
    @State private var selectedIssue: Issue?
    @State private var showDetailView: Bool = false
    
    var body: some View {
        ZStack {
            if isLoading {
                LoadingState(state: .loading, retryAction: nil)
            } else if let error = errorMessage {
                LoadingState(state: .error(message: error), retryAction: loadIssues)
            } else if issues.isEmpty {
                LoadingState(state: .empty(message: "No issues found"), retryAction: nil)
            } else {
                List(issues) { issue in
                    NavigationLink(destination: IssueDetailView(issue: issue)) {
                        IssueRowView(issue: issue)
                    }
                    .listRowInsets(EdgeInsets())
                    .listRowSeparator(.hidden)
                }
                .listStyle(.plain)
                .refreshable {
                    await refreshIssues()
                }
            }
        }
        .navigationTitle(repository.displayName)
        .navigationBarTitleDisplayMode(.inline)
        .onAppear {
            loadIssues()
        }
    }
    
    /// Load issues from FFI binding
    private func loadIssues() {
        isLoading = true
        errorMessage = nil
        
        Task {
            do {
                let fetchedIssues = try await FFIBridge.shared.fetchIssues(
                    owner: repository.owner,
                    repo: repository.name
                )
                await MainActor.run {
                    issues = fetchedIssues
                    isLoading = false
                }
            } catch {
                await MainActor.run {
                    errorMessage = error.localizedDescription
                    isLoading = false
                }
            }
        }
    }
    
    /// Refresh issues (pull-to-refresh)
    private func refreshIssues() async {
        isLoading = true
        errorMessage = nil
        
        do {
            let fetchedIssues = try await FFIBridge.shared.fetchIssues(
                owner: repository.owner,
                repo: repository.name
            )
            await MainActor.run {
                issues = fetchedIssues
                isLoading = false
            }
        } catch {
            await MainActor.run {
                errorMessage = error.localizedDescription
                isLoading = false
            }
        }
    }
}

#Preview {
    NavigationView {
        IssueListView(repository: Repository(
            id: "1",
            name: "aptu",
            owner: "clouatre-labs",
            url: "https://github.com/clouatre-labs/aptu",
            description: "Gamified OSS issue triage with AI assistance"
        ))
    }
}
