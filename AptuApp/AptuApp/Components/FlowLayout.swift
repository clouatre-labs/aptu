// SPDX-License-Identifier: Apache-2.0
//
// FlowLayout.swift
// Aptu
//
// Simple flow layout component for wrapping views

import SwiftUI

/// A simple flow layout that wraps content vertically
struct FlowLayout<Content: View>: View {
    let spacing: CGFloat
    let content: Content
    
    init(spacing: CGFloat = 8, @ViewBuilder content: () -> Content) {
        self.spacing = spacing
        self.content = content()
    }
    
    var body: some View {
        VStack(alignment: .leading, spacing: spacing) {
            content
        }
    }
}
