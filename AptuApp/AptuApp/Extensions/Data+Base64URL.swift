// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 Block, Inc.

import Foundation

/// RFC 7636 base64URL encoding/decoding extension for PKCE
extension Data {
    /// Encode data to RFC 7636 base64URL format
    /// 
    /// Converts standard base64 to base64URL by:
    /// - Replacing '+' with '-'
    /// - Replacing '/' with '_'
    /// - Removing padding ('=')
    ///
    /// - Returns: RFC 7636 compliant base64URL encoded string
    func base64URLEncoded() -> String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
    
    /// Decode RFC 7636 base64URL format to data
    ///
    /// Converts base64URL back to standard base64 by:
    /// - Replacing '-' with '+'
    /// - Replacing '_' with '/'
    /// - Adding padding ('=') as needed
    ///
    /// - Parameter string: RFC 7636 compliant base64URL encoded string
    /// - Returns: Decoded data, or nil if decoding fails
    static func base64URLDecoded(_ string: String) -> Data? {
        var base64 = string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        
        // Add padding if needed
        let padding = 4 - (base64.count % 4)
        if padding != 4 {
            base64.append(String(repeating: "=", count: padding))
        }
        
        return Data(base64Encoded: base64)
    }
}
