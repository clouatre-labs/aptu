// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import SwiftUI

struct ContentView: View {
    @State private var isAuthenticated: Bool = false
    
    var body: some View {
        if isAuthenticated {
            mainAppContent
        } else {
            AuthenticationView()
                .onDisappear {
                    // Check if authentication was successful
                    // This would be updated when the auth flow completes
                }
        }
    }
    
    private var mainAppContent: some View {
        VStack(spacing: 20) {
            Text("Aptu iOS App")
                .font(.largeTitle)
                .fontWeight(.bold)
            
            Text("GitHub Issue Triage Assistant")
                .font(.subheadline)
                .foregroundColor(.gray)
            
            Divider()
            
            VStack(alignment: .leading, spacing: 12) {
                Text("Welcome!")
                    .font(.headline)
                
                Text("You are now authenticated with GitHub. You can browse and triage issues from curated repositories.")
                    .font(.body)
                    .foregroundColor(.secondary)
            }
            .padding()
            .background(Color(.systemGray6))
            .cornerRadius(8)
            
            Spacer()
            
            Button(role: .destructive, action: {
                isAuthenticated = false
            }) {
                Text("Sign Out")
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.red.opacity(0.1))
                    .foregroundColor(.red)
                    .cornerRadius(8)
            }
        }
        .padding()
    }
}

#Preview {
    ContentView()
}
