//
//  LabelBadge.swift
//  AptuApp
//
//  SPDX-License-Identifier: Apache-2.0
//
//  Reusable component for displaying issue labels with color coding.
//

import SwiftUI

struct LabelBadge: View {
    let label: IssueLabel
    
    var backgroundColor: Color {
        colorFromHex(label.color)
    }
    
    var textColor: Color {
        // Determine if text should be light or dark based on background brightness
        let brightness = calculateBrightness(hex: label.color)
        return brightness > 0.5 ? .black : .white
    }
    
    var body: some View {
        Text(label.name)
            .font(.caption)
            .fontWeight(.semibold)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(backgroundColor)
            .foregroundColor(textColor)
            .cornerRadius(4)
    }
    
    /// Convert hex color string to SwiftUI Color
    private func colorFromHex(_ hex: String) -> Color {
        var cleanHex = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        
        // Handle 3-digit hex format by expanding to 6-digit
        if cleanHex.count == 3 {
            cleanHex = cleanHex.map { String($0) + String($0) }.joined()
        }
        
        let scanner = Scanner(string: cleanHex)
        var rgb: UInt64 = 0
        
        guard scanner.scanHexInt64(&rgb) else {
            return Color.gray // Fallback for invalid hex
        }
        
        let r = Double((rgb >> 16) & 0xFF) / 255.0
        let g = Double((rgb >> 8) & 0xFF) / 255.0
        let b = Double(rgb & 0xFF) / 255.0
        
        return Color(red: r, green: g, blue: b)
    }
    
    /// Calculate brightness of a hex color (0.0 = dark, 1.0 = light)
    private func calculateBrightness(hex: String) -> Double {
        var cleanHex = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        
        // Handle 3-digit hex format by expanding to 6-digit
        if cleanHex.count == 3 {
            cleanHex = cleanHex.map { String($0) + String($0) }.joined()
        }
        
        let scanner = Scanner(string: cleanHex)
        var rgb: UInt64 = 0
        
        guard scanner.scanHexInt64(&rgb) else {
            return 0.5 // Fallback for invalid hex
        }
        
        let r = Double((rgb >> 16) & 0xFF) / 255.0
        let g = Double((rgb >> 8) & 0xFF) / 255.0
        let b = Double(rgb & 0xFF) / 255.0
        
        // Standard brightness formula
        return (r * 0.299 + g * 0.587 + b * 0.114)
    }
}

#Preview {
    VStack(spacing: 8) {
        HStack(spacing: 8) {
            LabelBadge(label: IssueLabel(id: "1", name: "bug", color: "FF0000", description: "Something isn't working"))
            LabelBadge(label: IssueLabel(id: "2", name: "enhancement", color: "00FF00", description: "New feature"))
            LabelBadge(label: IssueLabel(id: "3", name: "documentation", color: "0000FF", description: "Improvements or additions to documentation"))
        }
        
        HStack(spacing: 8) {
            LabelBadge(label: IssueLabel(id: "4", name: "help wanted", color: "FFFF00", description: "Extra attention is needed"))
            LabelBadge(label: IssueLabel(id: "5", name: "good first issue", color: "90EE90", description: "Good for newcomers"))
        }
    }
    .padding()
}
