// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared.models

import kotlinx.serialization.Serializable

@Serializable
data class IssueLabel(
    val name: String,
    val color: String,
)

@Serializable
data class Repo(
    val id: String,
    val owner: String,
    val name: String,
    val description: String,
) {
    companion object {
        fun fromFfi(ffiRepo: Any): Repo {
            // TODO: Map from FfiRepo type once UniFFI bindings are generated
            // For now, this is a placeholder that will be filled in after Gobley generates the bindings
            throw NotImplementedError("Awaiting UniFFI code generation")
        }
    }
}

@Serializable
data class Issue(
    val id: String,
    val number: Int,
    val title: String,
    val body: String,
    val author: String,
    val createdAt: String,
    val labels: List<IssueLabel>,
    val repoOwner: String,
    val repoName: String,
    val url: String,
) {
    companion object {
        fun fromFfi(ffiIssue: Any): Issue {
            // TODO: Map from FfiIssue type once UniFFI bindings are generated
            throw NotImplementedError("Awaiting UniFFI code generation")
        }
    }
}

@Serializable
data class TriageResult(
    val labels: List<String>,
    val summary: String,
    val confidence: Double,
) {
    companion object {
        fun fromFfi(ffiResult: Any): TriageResult {
            // TODO: Map from FfiTriageResult type once UniFFI bindings are generated
            throw NotImplementedError("Awaiting UniFFI code generation")
        }
    }
}
