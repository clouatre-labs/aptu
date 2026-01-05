//
//  IssueRowView.swift
//  AptuApp
//
//  SPDX-License-Identifier: Apache-2.0
//
//  Reusable row component for issue list items.
//

import SwiftUI

struct IssueRowView: View {
    let issue: Issue
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Title and repository
            VStack(alignment: .leading, spacing: 4) {
                Text(issue.title)
                    .font(.headline)
                    .lineLimit(2)
                
                Text(issue.displayRepository)
                    .font(.caption)
                    .foregroundColor(.gray)
            }
            
            // Body preview
            Text(issue.bodyPreview)
                .font(.body)
                .foregroundColor(.gray)
                .lineLimit(2)
            
            // Labels and metadata
            VStack(alignment: .leading, spacing: 8) {
                // Labels
                if !issue.labels.isEmpty {
                    FlowLayout(spacing: 6) {
                        ForEach(issue.labels) { label in
                            LabelBadge(label: label)
                        }
                    }
                }
                
                // Metadata
                HStack(spacing: 12) {
                    HStack(spacing: 4) {
                        Image(systemName: "person.fill")
                            .font(.caption)
                        Text(issue.author)
                            .font(.caption)
                    }
                    .foregroundColor(.gray)
                    
                    Spacer()
                    
                    HStack(spacing: 4) {
                        Image(systemName: "calendar")
                            .font(.caption)
                        Text(formatDate(issue.updatedAt))
                            .font(.caption)
                    }
                    .foregroundColor(.gray)
                }
            }
        }
        .padding()
        .background(Color(.systemBackground))
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color(.systemGray5), lineWidth: 1)
        )
    }
    
    /// Format ISO date string to relative format
    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        guard let date = formatter.date(from: dateString) else {
            return "Unknown"
        }
        
        let calendar = Calendar.current
        let now = Date()
        let components = calendar.dateComponents([.day, .hour, .minute], from: date, to: now)
        
        if let days = components.day, days > 0 {
            return days == 1 ? "1 day ago" : "\(days) days ago"
        } else if let hours = components.hour, hours > 0 {
            return hours == 1 ? "1 hour ago" : "\(hours) hours ago"
        } else if let minutes = components.minute, minutes > 0 {
            return minutes == 1 ? "1 minute ago" : "\(minutes) minutes ago"
        } else {
            return "Just now"
        }
    }
}

// MARK: - Flow Layout Helper

struct FlowLayout: View {
    let spacing: CGFloat
    let items: [AnyView]
    
    init<Data: RandomAccessCollection>(spacing: CGFloat = 8, @ViewBuilder content: () -> [Data.Element]? = { nil }, @ViewBuilder builder: (Data.Element) -> some View) where Data.Element: Identifiable {
        self.spacing = spacing
        self.items = []
    }
    
    init(spacing: CGFloat = 8, @ViewBuilder content: () -> some View) {
        self.spacing = spacing
        self.items = []
    }
    
    var body: some View {
        VStack(alignment: .leading, spacing: spacing) {
            // Simplified flow layout - just use HStack with wrapping
            HStack(spacing: spacing) {
                ForEach(items, id: \.self) { item in
                    item
                }
            }
        }
    }
}

// Simpler FlowLayout implementation
struct SimpleFlowLayout<Content: View>: View {
    let spacing: CGFloat
    let content: Content
    
    init(spacing: CGFloat = 8, @ViewBuilder content: () -> Content) {
        self.spacing = spacing
        self.content = content()
    }
    
    var body: some View {
        VStack(alignment: .leading, spacing: spacing) {
            content
        }
    }
}

#Preview {
    VStack(spacing: 12) {
        IssueRowView(issue: Issue(
            id: "1",
            number: 42,
            title: "Add support for dark mode",
            body: "This issue requests adding dark mode support to the application. Users have been asking for this feature for a while.",
            author: "john-doe",
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-03T12:30:00Z",
            labels: [
                IssueLabel(id: "1", name: "enhancement", color: "00FF00", description: "New feature"),
                IssueLabel(id: "2", name: "ui", color: "0000FF", description: "User interface")
            ],
            repositoryName: "aptu",
            repositoryOwner: "clouatre-labs",
            url: "https://github.com/clouatre-labs/aptu/issues/42"
        ))
        
        IssueRowView(issue: Issue(
            id: "2",
            number: 43,
            title: "Fix crash on startup",
            body: "The app crashes when launched on iOS 17.",
            author: "jane-smith",
            createdAt: "2024-01-02T00:00:00Z",
            updatedAt: "2024-01-02T15:45:00Z",
            labels: [
                IssueLabel(id: "3", name: "bug", color: "FF0000", description: "Something isn't working")
            ],
            repositoryName: "aptu",
            repositoryOwner: "clouatre-labs",
            url: "https://github.com/clouatre-labs/aptu/issues/43"
        ))
    }
    .padding()
}
