//
//  LoadingState.swift
//  AptuApp
//
//  SPDX-License-Identifier: Apache-2.0
//
//  Reusable component for displaying loading, error, and empty states.
//

import SwiftUI

struct LoadingState: View {
    enum State {
        case loading
        case empty(message: String)
        case error(message: String)
    }
    
    let state: State
    let retryAction: (() -> Void)?
    
    var body: some View {
        VStack(spacing: 16) {
            switch state {
            case .loading:
                ProgressView()
                    .scaleEffect(1.5)
                Text("Loading...")
                    .font(.headline)
                    .foregroundColor(.gray)
                
            case .empty(let message):
                Image(systemName: "tray")
                    .font(.system(size: 48))
                    .foregroundColor(.gray)
                Text(message)
                    .font(.headline)
                    .foregroundColor(.gray)
                    .multilineTextAlignment(.center)
                
            case .error(let message):
                Image(systemName: "exclamationmark.triangle")
                    .font(.system(size: 48))
                    .foregroundColor(.red)
                Text("Error")
                    .font(.headline)
                    .foregroundColor(.red)
                Text(message)
                    .font(.body)
                    .foregroundColor(.gray)
                    .multilineTextAlignment(.center)
                
                if let retryAction = retryAction {
                    Button(action: retryAction) {
                        Text("Retry")
                            .frame(maxWidth: .infinity)
                            .padding()
                            .background(Color.blue)
                            .foregroundColor(.white)
                            .cornerRadius(8)
                    }
                }
            }
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
    }
}

#Preview {
    VStack(spacing: 20) {
        LoadingState(state: .loading, retryAction: nil)
        
        LoadingState(state: .empty(message: "No issues found"), retryAction: nil)
        
        LoadingState(state: .error(message: "Failed to load issues"), retryAction: {})
    }
}
