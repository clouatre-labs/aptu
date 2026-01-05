//
//  ContentView.swift
//  AptuApp
//
//  SPDX-License-Identifier: Apache-2.0
//
//  Main content view orchestrating RepositoryPickerView and IssueListView.
//

import SwiftUI

struct ContentView: View {
    @State private var selectedRepository: Repository?
    @State private var navigationPath: [Repository] = []
    
    var body: some View {
        NavigationStack(path: $navigationPath) {
            if let repository = selectedRepository {
                IssueListView(repository: repository)
                    .navigationBarBackButtonHidden(false)
                    .toolbar {
                        ToolbarItem(placement: .navigationBarLeading) {
                            Button(action: {
                                selectedRepository = nil
                                navigationPath.removeAll()
                            }) {
                                HStack(spacing: 4) {
                                    Image(systemName: "chevron.left")
                                    Text("Back")
                                }
                            }
                        }
                    }
            } else {
                RepositoryPickerView { repository in
                    selectedRepository = repository
                    navigationPath.append(repository)
                }
            }
        }
    }
}

#Preview {
    ContentView()
}
