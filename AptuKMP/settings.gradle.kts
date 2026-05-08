// SPDX-License-Identifier: Apache-2.0

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
include(":androidApp")
