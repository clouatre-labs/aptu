// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import SwiftUI

struct ContentView: View {
    @State private var selectedRepository: Repository?
    
    var body: some View {
        if let repository = selectedRepository {
            NavigationView {
                IssueListView(repository: repository)
                    .navigationBarBackButtonHidden(false)
                    .toolbar {
                        ToolbarItem(placement: .navigationBarLeading) {
                            Button(action: {
                                selectedRepository = nil
                            }) {
                                HStack(spacing: 4) {
                                    Image(systemName: "chevron.left")
                                    Text("Back")
                                }
                            }
                        }
                    }
            }
        } else {
            RepositoryPickerView { repository in
                selectedRepository = repository
            }
        }
    }
}

#Preview {
    ContentView()
}
