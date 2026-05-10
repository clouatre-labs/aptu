// SPDX-License-Identifier: Apache-2.0

import gobley.gradle.GobleyHost

plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.library)
    alias(libs.plugins.gobley.cargo)
    alias(libs.plugins.gobley.uniffi)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.kotlin.atomicfu)
}

kotlin {
    androidTarget()

    sourceSets {
        commonMain.dependencies {
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
