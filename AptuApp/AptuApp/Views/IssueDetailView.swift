//
//  IssueDetailView.swift
//  AptuApp
//
//  SPDX-License-Identifier: Apache-2.0
//
//  Detailed issue view showing full issue metadata with action buttons.
//

import SwiftUI

struct IssueDetailView: View {
    @Environment(\.openURL) var openURL
    
    let issue: Issue
    
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // Title
                VStack(alignment: .leading, spacing: 8) {
                    Text(issue.title)
                        .font(.title2)
                        .fontWeight(.bold)
                    
                    Text(issue.displayRepository)
                        .font(.caption)
                        .foregroundColor(.gray)
                }
                
                Divider()
                
                // Metadata
                VStack(alignment: .leading, spacing: 12) {
                    MetadataRow(label: "Author", value: issue.author)
                    MetadataRow(label: "Created", value: formatDate(issue.createdAt))
                    MetadataRow(label: "Updated", value: formatDate(issue.updatedAt))
                    MetadataRow(label: "Issue #", value: String(issue.number))
                }
                
                Divider()
                
                // Labels
                if !issue.labels.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Labels")
                            .font(.headline)
                        
                        FlowLayoutView {
                            ForEach(issue.labels) { label in
                                LabelBadge(label: label)
                            }
                        }
                    }
                    
                    Divider()
                }
                
                // Body
                VStack(alignment: .leading, spacing: 8) {
                    Text("Description")
                        .font(.headline)
                    
                    if let body = issue.body, !body.isEmpty {
                        Text(body)
                            .font(.body)
                            .foregroundColor(.gray)
                            .textSelection(.enabled)
                    } else {
                        Text("No description provided")
                            .font(.body)
                            .foregroundColor(.gray)
                            .italic()
                    }
                }
                
                Divider()
                
                // Action Buttons
                VStack(spacing: 12) {
                    Button(action: {
                        openURL(URL(string: issue.url)!)
                    }) {
                        HStack {
                            Image(systemName: "safari")
                            Text("Open in GitHub")
                        }
                        .frame(maxWidth: .infinity)
                        .padding()
                        .background(Color.blue)
                        .foregroundColor(.white)
                        .cornerRadius(8)
                    }
                    
                    Button(action: copyLink) {
                        HStack {
                            Image(systemName: "doc.on.doc")
                            Text("Copy Link")
                        }
                        .frame(maxWidth: .infinity)
                        .padding()
                        .background(Color(.systemGray4))
                        .foregroundColor(.primary)
                        .cornerRadius(8)
                    }
                }
                
                Spacer()
            }
            .padding()
        }
        .navigationTitle("Issue Details")
        .navigationBarTitleDisplayMode(.inline)
    }
    
    /// Copy issue link to clipboard
    private func copyLink() {
        UIPasteboard.general.string = issue.url
    }
    
    /// Format ISO date string to readable format
    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        guard let date = formatter.date(from: dateString) else {
            return "Unknown"
        }
        
        let dateFormatter = DateFormatter()
        dateFormatter.dateStyle = .medium
        dateFormatter.timeStyle = .short
        return dateFormatter.string(from: date)
    }
}

// MARK: - Helper Views

struct MetadataRow: View {
    let label: String
    let value: String
    
    var body: some View {
        HStack {
            Text(label)
                .font(.caption)
                .foregroundColor(.gray)
                .frame(width: 80, alignment: .leading)
            
            Text(value)
                .font(.body)
                .fontWeight(.semibold)
            
            Spacer()
        }
    }
}

struct FlowLayoutView<Content: View>: View {
    let content: Content
    
    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            content
        }
    }
}

#Preview {
    NavigationView {
        IssueDetailView(issue: Issue(
            id: "1",
            number: 42,
            title: "Add support for dark mode",
            body: "This issue requests adding dark mode support to the application. Users have been asking for this feature for a while.\n\nImplementation should:\n- Use system appearance settings\n- Support both light and dark modes\n- Maintain accessibility standards",
            author: "john-doe",
            createdAt: "2024-01-01T10:30:00Z",
            updatedAt: "2024-01-03T14:45:00Z",
            labels: [
                IssueLabel(id: "1", name: "enhancement", color: "00FF00", description: "New feature"),
                IssueLabel(id: "2", name: "ui", color: "0000FF", description: "User interface"),
                IssueLabel(id: "3", name: "good first issue", color: "90EE90", description: "Good for newcomers")
            ],
            repositoryName: "aptu",
            repositoryOwner: "clouatre-labs",
            url: "https://github.com/clouatre-labs/aptu/issues/42"
        ))
    }
}
