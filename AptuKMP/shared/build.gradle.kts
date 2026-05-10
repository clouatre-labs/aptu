// SPDX-License-Identifier: Apache-2.0

plugins {
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.android.library)
    alias(libs.plugins.gobley.cargo)
    alias(libs.plugins.gobley.uniffi)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.kotlin.atomicfu)
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

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21)
    }
    sourceSets {
        main {
            dependencies {
                implementation(libs.coroutines.core)
                implementation(libs.coroutines.android)
                implementation(libs.ktor.client.core)
                implementation(libs.ktor.client.android)
                implementation(libs.kotlinx.serialization.json)
            }
        }
        test {
            dependencies {
                implementation(kotlin("test"))
                implementation(libs.coroutines.test)
            }
        }
    }
}

cargo {
    packageDirectory = layout.projectDirectory.dir("../../crates/aptu-ffi")
}

uniffi {
    generateFromLibrary()
}
