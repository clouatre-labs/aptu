// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import SwiftUI

struct ContentView: View {
    @State private var navigationPath: [Repository] = []
    @State private var showSettings = false
    
    var body: some View {
        NavigationStack(path: $navigationPath) {
            RepositoryPickerView { repository in
                navigationPath.append(repository)
            }
            .navigationDestination(for: Repository.self) { repository in
                IssueListView(repository: repository)
            }
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button {
                        showSettings = true
                    } label: {
                        Image(systemName: "gear")
                    }
                }
            }
            .sheet(isPresented: $showSettings) {
                OpenRouterAuthView()
            }
        }
    }
}

#Preview {
    ContentView()
}
