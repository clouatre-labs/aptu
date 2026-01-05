import Foundation
import Security

/// SwiftKeychainProvider conforms to the UniFFI-generated KeychainProvider protocol
/// This bridges Swift's system keychain to the Rust FFI layer
final class SwiftKeychainProvider: KeychainProvider {
    private let keychain = SwiftKeychain.shared
    
    func getToken(service: String, account: String) throws -> String? {
        return keychain.getToken(service: service, account: account)
    }
    
    func setToken(service: String, account: String, token: String) throws {
        try keychain.setToken(service: service, account: account, token: token)
    }
    
    func deleteToken(service: String, account: String) throws {
        try keychain.deleteToken(service: service, account: account)
    }
}

final class SwiftKeychain: @unchecked Sendable {
    static let shared = SwiftKeychain()

    private let service = "com.aptu.app"

    func getToken(service: String, account: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        guard status == errSecSuccess,
              let data = result as? Data,
              let token = String(data: data, encoding: .utf8)
        else {
            return nil
        }

        return token
    }

    func setToken(service: String, account: String, token: String) throws {
        guard let data = token.data(using: .utf8) else {
            throw KeychainError.encodingFailed
        }

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]

        SecItemDelete(query as CFDictionary)

        let attributes: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: data,
        ]

        let status = SecItemAdd(attributes as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.storeFailed(status)
        }
    }

    func deleteToken(service: String, account: String) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]

        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeychainError.deleteFailed(status)
        }
    }
}

enum KeychainError: LocalizedError {
    case encodingFailed
    case storeFailed(OSStatus)
    case deleteFailed(OSStatus)
    case retrievalFailed(OSStatus)

    var errorDescription: String? {
        switch self {
        case .encodingFailed:
            return "Failed to encode token"
        case .storeFailed(let status):
            return "Failed to store token (status: \(status))"
        case .deleteFailed(let status):
            return "Failed to delete token (status: \(status))"
        case .retrievalFailed(let status):
            return "Failed to retrieve token (status: \(status))"
        }
    }
}
