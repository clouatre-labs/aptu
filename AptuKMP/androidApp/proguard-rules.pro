# SPDX-FileCopyrightText: 2026 Hugues Clouâtre <hugues@clouatre.dev>
# SPDX-License-Identifier: Apache-2.0

# Keep UniFFI-generated bindings (Gobley generates these at build time)
-keep class dev.aptu.ffi.** { *; }

# Keep Kotlin Serialization
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt
-keepclassmembers class kotlinx.serialization.json.** { *** Companion; }
-keepclasseswithmembers class kotlinx.serialization.** { kotlinx.serialization.KSerializer serializer(...); }

# Keep KVault (uses reflection for EncryptedSharedPreferences)
-keep class com.liftric.kvault.** { *; }

# Keep Ktor internals
-keep class io.ktor.** { *; }
-keep class kotlinx.coroutines.** { *; }
