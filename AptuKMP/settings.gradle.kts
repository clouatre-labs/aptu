// SPDX-FileCopyrightText: 2026 Aptu Contributors
// SPDX-License-Identifier: Apache-2.0

// Note: pluginManagement and dependencyResolutionManagement blocks execute in an isolated
// early phase of Gradle settings evaluation. Top-level vals defined outside those blocks
// are not in scope inside them, so the URL must be inlined in each block.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
        maven("https://gitlab.com/gobley/gobley/-/packages/maven")
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        maven("https://gitlab.com/gobley/gobley/-/packages/maven")
    }
}

rootProject.name = "AptuKMP"
include(":shared")
