import SwiftUI

struct ContentView: View {
    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                Text("Aptu")
                    .font(.largeTitle)
                    .fontWeight(.bold)

                Text("AI-Powered OSS Issue Triage")
                    .font(.subheadline)
                    .foregroundColor(.secondary)

                Spacer()

                VStack(spacing: 12) {
                    Button(action: {}) {
                        Label("Browse Issues", systemImage: "list.bullet")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)

                    Button(action: {}) {
                        Label("My Contributions", systemImage: "checkmark.circle")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)

                    Button(action: {}) {
                        Label("Settings", systemImage: "gear")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                }

                Spacer()

                Text("Placeholder UI - Full implementation in next PR")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .padding()
            .navigationTitle("Aptu")
        }
    }
}

#Preview {
    ContentView()
}
