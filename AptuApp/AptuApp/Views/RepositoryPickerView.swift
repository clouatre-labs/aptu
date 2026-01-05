//
//  RepositoryPickerView.swift
//  AptuApp
//
//  SPDX-License-Identifier: Apache-2.0
//
//  Repository selection view using list_curated_repos FFI binding.
//

import SwiftUI

struct RepositoryPickerView: View {
    @State private var repositories: [Repository] = []
    @State private var filteredRepositories: [Repository] = []
    @State private var searchText: String = ""
    @State private var isLoading: Bool = false
    @State private var errorMessage: String?
    
    let onRepositorySelected: (Repository) -> Void
    
    var body: some View {
        ZStack {
            if isLoading {
                LoadingState(state: .loading, retryAction: nil)
            } else if let error = errorMessage {
                LoadingState(state: .error(message: error), retryAction: loadRepositories)
            } else if filteredRepositories.isEmpty {
                LoadingState(state: .empty(message: "No repositories found"), retryAction: nil)
            } else {
                List(filteredRepositories) { repo in
                    Button(action: {
                        onRepositorySelected(repo)
                    }) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(repo.displayName)
                                .font(.headline)
                                .foregroundColor(.primary)
                            
                            if let description = repo.description {
                                Text(description)
                                    .font(.caption)
                                    .foregroundColor(.gray)
                                    .lineLimit(2)
                            }
                        }
                        .padding(.vertical, 8)
                    }
                }
                .searchable(text: $searchText, prompt: "Search repositories")
                .onChange(of: searchText) { _, newValue in
                    filterRepositories(newValue)
                }
            }
        }
        .navigationTitle("Select Repository")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear {
            loadRepositories()
        }
    }
    
    /// Load repositories from FFI binding
    private func loadRepositories() {
        isLoading = true
        errorMessage = nil
        
        Task {
            do {
                let repos = try await FFIBridge.shared.listCuratedRepositories()
                await MainActor.run {
                    repositories = repos
                    filteredRepositories = repos
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
    
    /// Filter repositories based on search text
    private func filterRepositories(_ searchText: String) {
        if searchText.isEmpty {
            filteredRepositories = repositories
        } else {
            let lowercased = searchText.lowercased()
            filteredRepositories = repositories.filter { repo in
                repo.displayName.lowercased().contains(lowercased) ||
                (repo.description?.lowercased().contains(lowercased) ?? false)
            }
        }
    }
}

#Preview {
    RepositoryPickerView { repo in
        print("Selected: \(repo.displayName)")
    }
}
