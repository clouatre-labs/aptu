//
//  ContentView.swift
//  AptuApp
//
//  Main UI view demonstrating Rust FFI integration with sample function calls.
//

import SwiftUI

struct ContentView: View {
    @State private var output: String = "Ready to call Rust FFI..."
    @State private var isLoading: Bool = false
    
    var body: some View {
        VStack(spacing: 20) {
            Text("Aptu iOS App")
                .font(.largeTitle)
                .fontWeight(.bold)
            
            Text("Rust FFI Integration Demo")
                .font(.subheadline)
                .foregroundColor(.gray)
            
            Divider()
            
            VStack(alignment: .leading, spacing: 12) {
                Text("Output:")
                    .font(.headline)
                
                ScrollView {
                    Text(output)
                        .font(.system(.body, design: .monospaced))
                        .padding()
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color(.systemGray6))
                        .cornerRadius(8)
                }
                .frame(minHeight: 150)
            }
            
            VStack(spacing: 12) {
                Button(action: callRustFFI) {
                    HStack {
                        if isLoading {
                            ProgressView()
                                .tint(.white)
                        }
                        Text("Call Rust FFI")
                    }
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.blue)
                    .foregroundColor(.white)
                    .cornerRadius(8)
                }
                .disabled(isLoading)
                
                Button(action: clearOutput) {
                    Text("Clear")
                        .frame(maxWidth: .infinity)
                        .padding()
                        .background(Color(.systemGray4))
                        .foregroundColor(.black)
                        .cornerRadius(8)
                }
            }
            
            Spacer()
        }
        .padding()
    }
    
    /// Call Rust FFI function
    private func callRustFFI() {
        isLoading = true
        
        DispatchQueue.global().async {
            // Example: Call Rust FFI function
            // This would use the UniFFI-generated bindings
            // For example: let result = aptuFFI.someFunction()
            
            let result = "Rust FFI call successful!\n\nThis demonstrates the integration between Swift and Rust via UniFFI bindings."
            
            DispatchQueue.main.async {
                output = result
                isLoading = false
            }
        }
    }
    
    /// Clear output
    private func clearOutput() {
        output = "Ready to call Rust FFI..."
    }
}

#Preview {
    ContentView()
}
