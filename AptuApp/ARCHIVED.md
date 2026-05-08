# AptuApp - Archived

The SwiftUI prototype in this directory is archived. The active mobile project is now **AptuKMP** (Kotlin Multiplatform), which provides:

- Cross-platform UI (iOS + Android) via Compose Multiplatform
- Unified codebase for business logic (Kotlin commonMain)
- Direct integration with aptu-ffi Rust bindings via UniFFI + Gobley
- Secure credential storage via KVault (Android Keychain + iOS Keychain)

See `../AptuKMP/README.md` for setup and architecture.

The SwiftUI prototype served as a proof-of-concept for the mobile experience. KMP is the production path forward.
