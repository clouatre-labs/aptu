// SPDX-License-Identifier: Apache-2.0

import gobley.gradle.GobleyHost

plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.library)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.gobley.cargo)
    alias(libs.plugins.gobley.uniffi)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.kotlin.atomicfu)
}

kotlin {
    androidTarget()

    // iOS targets are only available when building on macOS (Gobley requirement).
    if (GobleyHost.Platform.MacOS.isCurrent) {
        iosArm64 {
            binaries.framework {
                baseName = "shared"
                isStatic = true
            }
        }

        iosSimulatorArm64 {
            binaries.framework {
                baseName = "shared"
                isStatic = true
            }
        }
    }

    sourceSets {
        commonMain.dependencies {
            implementation(compose.runtime)
            implementation(compose.foundation)
            implementation(compose.material3)
            implementation(libs.coroutines.core)
            implementation(libs.ktor.client.core)
            implementation(libs.kotlinx.serialization.json)
        }

        commonTest.dependencies {
            implementation(kotlin("test"))
            implementation(libs.coroutines.test)
        }

        androidMain.dependencies {
            implementation(libs.androidx.activity.compose)
            implementation(libs.coroutines.android)
            implementation(libs.ktor.client.android)
        }

        if (GobleyHost.Platform.MacOS.isCurrent) {
            iosMain.dependencies {
                implementation(libs.kvault)
                implementation(libs.ktor.client.darwin)
            }
        }
    }
}

android {
    namespace = "dev.aptu.shared"
    compileSdk = 35

    defaultConfig {
        minSdk = 26
        ndk {
            abiFilters.addAll(listOf("arm64-v8a", "x86_64"))
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }
}

cargo {
    // packageDirectory is relative to this build file (shared/).
    // aptu-ffi lives two levels up at the workspace root under crates/.
    packageDirectory = layout.projectDirectory.dir("../../crates/aptu-ffi")
}

uniffi {
    generateFromLibrary()
}
