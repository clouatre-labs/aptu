# AptuKMP - Kotlin Multiplatform Mobile

Cross-platform mobile client for Aptu (AI-powered OSS issue triage and PR review) using Kotlin Multiplatform (KMP) with Compose Multiplatform UI.

## Architecture

- **shared/** - Kotlin Multiplatform library with business logic, ViewModels, and domain models
  - **commonMain/** - Shared code (ViewModels, FFI wrappers, models)
  - **androidMain/** - Android-specific implementations (Keychain via KVault)
  - **iosMain/** - iOS-specific implementations (Keychain via KVault)
  - **commonTest/** - Shared unit tests

- **androidApp/** - Android application using Compose
  - Five screens: Auth, RepoPicker, IssueList, IssueDetail, Settings
  - Navigation via Jetpack Navigation Compose

- **iosApp/** - iOS application with SwiftUI host
  - Thin SwiftUI wrapper around Compose Multiplatform UI
  - Uses ComposeUIViewController for Compose integration

## Stack

- **Kotlin 2.1.0** - Language
- **Compose Multiplatform 1.8.0** - UI framework
- **Gobley 0.3.7** - Gradle plugin for Rust/UniFFI integration
- **KVault 1.12.0** - Cross-platform secure storage (Keychain)
- **Coroutines 1.8.1** - Async/await
- **Ktor Client** - HTTP client for GitHub OAuth device flow

## Setup

### Prerequisites

- Rust 1.70+ (stable)
- Kotlin 2.1.0
- Android SDK 35 (compileSdk)
- Xcode 15+ (for iOS)
- Java 21

### Build

```bash
cd AptuKMP

# Build shared library (generates UniFFI bindings)
./gradlew :shared:build

# Build Android app
./gradlew :androidApp:assembleDebug

# Build iOS app (requires Xcode)
cd iosApp
xcodebuild -scheme iosApp -destination 'platform=iOS Simulator,name=iPhone 16' build
```

### Test

```bash
cd AptuKMP

# Run all unit tests
./gradlew :shared:testDebugUnitTest :androidApp:testDebugUnitTest
```

## Key Design Decisions

### FFI Integration

- **Gobley Gradle Plugin** - Automates Rust compilation and UniFFI Kotlin binding generation
- **Dispatchers.IO** - All FFI calls dispatched to IO thread to avoid blocking main thread
- **Domain Models** - Ffi* types never leak into ViewModels or UI; always wrapped in clean Kotlin data classes

### Secure Storage

- **KVault** - Multiplatform library that abstracts Android Keychain and iOS Keychain
- **expect/actual** - KeychainProvider defined in commonMain, implemented in androidMain and iosMain
- Both implementations use KVault internally for consistency

### State Management

- **StateFlow** - Reactive state in ViewModels (commonMain)
- **Compose** - Observes StateFlow via collectAsState()
- No AndroidX ViewModel in commonMain (not multiplatform); plain classes with StateFlow

### Navigation

- **Android** - Jetpack Navigation Compose with typed routes
- **iOS** - Placeholder; full Compose navigation wired in shared code

## File Structure

```
AptuKMP/
├── settings.gradle.kts
├── build.gradle.kts
├── gradle/
│   └── libs.versions.toml
├── shared/
│   ├── build.gradle.kts
│   └── src/
│       ├── commonMain/kotlin/dev/aptu/shared/
│       │   ├── AptuFfi.kt
│       │   ├── KeychainProvider.kt
│       │   ├── models/Models.kt
│       │   └── viewmodels/
│       │       ├── AuthViewModel.kt
│       │       ├── RepoViewModel.kt
│       │       └── IssueViewModel.kt
│       ├── androidMain/kotlin/dev/aptu/shared/
│       │   └── KeychainProvider.android.kt
│       ├── iosMain/kotlin/dev/aptu/shared/
│       │   ├── KeychainProvider.ios.kt
│       │   └── MainViewController.kt
│       └── commonTest/kotlin/dev/aptu/shared/
│           ├── KeychainProviderTest.kt
│           └── RepoViewModelTest.kt
├── androidApp/
│   ├── build.gradle.kts
│   └── src/main/
│       ├── AndroidManifest.xml
│       ├── kotlin/dev/aptu/android/
│       │   ├── MainActivity.kt
│       │   └── ui/
│       │       ├── App.kt
│       │       ├── AuthScreen.kt
│       │       ├── RepoPickerScreen.kt
│       │       ├── IssueListScreen.kt
│       │       ├── IssueDetailScreen.kt
│       │       └── SettingsScreen.kt
│       └── res/
├── iosApp/
│   ├── iosApp.xcodeproj/
│   └── iosApp/
│       ├── iOSApp.swift
│       └── ContentView.swift
└── .github/workflows/
    ├── android.yml
    └── ios.yml
```

## Known Limitations

- **GitHub OAuth Device Flow** - Placeholder implementation in AuthViewModel; requires HTTP client setup
- **iOS Xcode Project** - Minimal setup; full Compose navigation not yet wired
- **FFI Bindings** - Depend on Gobley code generation; placeholders in AptuFfi.kt until bindings are available

## Next Steps

1. Implement GitHub OAuth device flow in AuthViewModel (HTTP polling)
2. Wire full Compose navigation on iOS (currently placeholder)
3. Add instrumented tests for FFI threading (Android)
4. Integrate with aptu-ffi Rust crate once UniFFI bindings are generated
5. Add CI/CD for automated builds and testing

## License

Apache-2.0
