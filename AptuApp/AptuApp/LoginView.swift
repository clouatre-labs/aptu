//
//  LoginView.swift
//  AptuApp
//
//  GitHub authentication login screen.
//

import SwiftUI

struct LoginView: View {
    @State private var isLoading: Bool = false
    @State private var errorMessage: String?
    
    var body: some View {
        VStack(spacing: 20) {
            Spacer()
            
            VStack(spacing: 12) {
                Image(systemName: "lock.circle.fill")
                    .font(.system(size: 60))
                    .foregroundColor(.blue)
                
                Text("Aptu")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                
                Text("GitHub Authentication")
                    .font(.subheadline)
                    .foregroundColor(.gray)
            }
            
            Spacer()
            
            if let errorMessage = errorMessage {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Image(systemName: "exclamationmark.circle.fill")
                            .foregroundColor(.red)
                        Text(errorMessage)
                            .font(.caption)
                    }
                    .padding()
                    .background(Color(.systemRed).opacity(0.1))
                    .cornerRadius(8)
                }
            }
            
            VStack(spacing: 12) {
                Button(action: authenticateWithGitHub) {
                    HStack {
                        if isLoading {
                            ProgressView()
                                .tint(.white)
                        } else {
                            Image(systemName: "person.crop.circle.fill")
                        }
                        Text("Sign in with GitHub")
                    }
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.black)
                    .foregroundColor(.white)
                    .cornerRadius(8)
                }
                .disabled(isLoading)
                
                Text("You need to authenticate with GitHub to use Aptu")
                    .font(.caption)
                    .foregroundColor(.gray)
                    .multilineTextAlignment(.center)
            }
            
            Spacer()
        }
        .padding()
    }
    
    /// Authenticate with GitHub
    private func authenticateWithGitHub() {
        isLoading = true
        errorMessage = nil
        
        // TODO: Implement GitHub OAuth flow
        // This would typically involve:
        // 1. Opening a web view or browser for GitHub OAuth
        // 2. Handling the callback with the authorization code
        // 3. Exchanging the code for an access token
        // 4. Storing the token in the keychain
        
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            isLoading = false
            errorMessage = "GitHub authentication not yet implemented"
        }
    }
}

#Preview {
    LoginView()
}
