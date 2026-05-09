// SPDX-License-Identifier: Apache-2.0

plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.library)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.gobley.cargo)
    alias(libs.plugins.gobley.uniffi)
    alias(libs.plugins.kotlin.serialization)
}

kotlin {
    androidTarget()

    iosArm64 {
        binaries.framework {
            baseName = "shared"
        }
        compilations.main {
            useRustUpLinker()
        }
    }

    iosSimulatorArm64 {
        binaries.framework {
            baseName = "shared"
        }
        compilations.main {
            useRustUpLinker()
        }
    }

    sourceSets {
        commonMain.dependencies {
            implementation(compose.runtime)
            implementation(compose.foundation)
            implementation(compose.material3)
            implementation(libs.coroutines.core)
            implementation(libs.kvault)
            implementation(libs.ktor.client.core)
            implementation(libs.kotlinx.serialization.json)
        }

        androidMain.dependencies {
            implementation(libs.androidx.activity.compose)
            implementation(libs.coroutines.android)
            implementation(libs.ktor.client.android)
        }

        iosMain.dependencies {
            implementation(libs.ktor.client.darwin)
        }
    }
}

android {
    namespace = "dev.aptu.shared"
    compileSdk = 35

    defaultConfig {
        minSdk = 26
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }
}

cargo {
    // "release" is used for all builds including local development. The workspace
    // Cargo.toml defines a "ci" profile (inherits release, lto=false, codegen-units=16)
    // which is faster to compile. To use it locally: set profile = "ci" here and
    // run `cargo build --profile ci -p aptu-ffi` before the Gradle build.
    // We do not switch profiles per Gradle build type to keep the Gobley config simple.
    packageDirectory = layout.projectDirectory.dir("../../crates/aptu-ffi")
    profile = "release"
    targets = listOf("aarch64-linux-android", "x86_64-linux-android", "aarch64-apple-ios", "aarch64-apple-ios-sim")
}

uniffi {
    generateFromLibrary {
        name = "aptu_ffi"
    }
}
