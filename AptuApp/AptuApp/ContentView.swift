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
