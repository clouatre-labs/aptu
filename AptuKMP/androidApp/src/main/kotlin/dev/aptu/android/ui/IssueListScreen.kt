// SPDX-License-Identifier: Apache-2.0

package dev.aptu.android.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.aptu.shared.aptuKeychain
import dev.aptu.shared.models.Issue
import dev.aptu.shared.viewmodels.IssueState
import dev.aptu.shared.viewmodels.IssueViewModel

@Composable
fun IssueListScreen(
    owner: String,
    repo: String,
    viewModel: IssueViewModel,
    onIssueSelected: (issueId: String) -> Unit,
    onNavigateBack: () -> Unit,
) {
    val state = viewModel.state.collectAsState()
    val keychain = aptuKeychain()

    LaunchedEffect(owner, repo) {
        viewModel.load(keychain)
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
        ) {
            IconButton(onClick = onNavigateBack) {
                Icon(Icons.Default.ArrowBack, contentDescription = "Back")
            }
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                text = "$owner/$repo",
                style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.weight(1f),
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        when (val currentState = state.value) {
            IssueState.Loading -> {
                Text("Loading issues...")
            }

            is IssueState.Success -> {
                LazyColumn {
                    items(currentState.issues) { issue ->
                        IssueItem(
                            issue = issue,
                            onClick = {
                                onIssueSelected(issue.id)
                            },
                        )
                    }
                }
            }

            is IssueState.Error -> {
                Text(
                    text = "Error: ${currentState.message}",
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}

@Composable
fun IssueItem(
    issue: Issue,
    onClick: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(12.dp),
    ) {
        Text(
            text = "#${issue.number} ${issue.title}",
            style = MaterialTheme.typography.titleMedium,
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = "by ${issue.author} on ${issue.createdAt}",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
