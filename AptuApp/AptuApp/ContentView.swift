// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import SwiftUI

struct ContentView: View {
    @State private var navigationPath: [Repository] = []
    
    var body: some View {
        NavigationStack(path: $navigationPath) {
            RepositoryPickerView { repository in
                navigationPath.append(repository)
            }
            .navigationDestination(for: Repository.self) { repository in
                IssueListView(repository: repository)
            }
        }
    }
}

#Preview {
    ContentView()
}
