// SPDX-License-Identifier: Apache-2.0
//
// FlowLayout.swift
// Aptu
//
// Flow layout component for wrapping views horizontally

import SwiftUI

/// A flow layout that wraps content horizontally using the Layout protocol
struct FlowLayout: Layout {
    let spacing: CGFloat
    
    init(spacing: CGFloat = 8) {
        self.spacing = spacing
    }
    
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard !subviews.isEmpty else { return .zero }
        
        var totalHeight: CGFloat = 0
        var currentLineWidth: CGFloat = 0
        var currentLineHeight: CGFloat = 0
        let maxWidth = proposal.width ?? .infinity
        
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            
            // Check if we need to wrap to next line
            if currentLineWidth + size.width + spacing > maxWidth && currentLineWidth > 0 {
                totalHeight += currentLineHeight + spacing
                currentLineWidth = size.width
                currentLineHeight = size.height
            } else {
                if currentLineWidth > 0 {
                    currentLineWidth += spacing
                }
                currentLineWidth += size.width
                currentLineHeight = max(currentLineHeight, size.height)
            }
        }
        
        // Add the last line
        totalHeight += currentLineHeight
        
        return CGSize(width: maxWidth, height: totalHeight)
    }
    
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        guard !subviews.isEmpty else { return }
        
        var currentX = bounds.minX
        var currentY = bounds.minY
        var lineHeight: CGFloat = 0
        let maxWidth = bounds.width
        
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            
            // Check if we need to wrap to next line
            if currentX + size.width > bounds.maxX && currentX > bounds.minX {
                currentY += lineHeight + spacing
                currentX = bounds.minX
                lineHeight = 0
            }
            
            // Place the subview
            let point = CGPoint(x: currentX, y: currentY)
            subview.place(at: point, proposal: ProposedViewSize(width: size.width, height: size.height))
            
            // Update position for next subview
            currentX += size.width + spacing
            lineHeight = max(lineHeight, size.height)
        }
    }
}
