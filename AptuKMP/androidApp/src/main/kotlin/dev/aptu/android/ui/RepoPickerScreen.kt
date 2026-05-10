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
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SearchBar
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.aptu.shared.models.Repo
import dev.aptu.shared.viewmodels.RepoState
import dev.aptu.shared.viewmodels.RepoViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RepoPickerScreen(
    viewModel: RepoViewModel,
    onRepoSelected: (owner: String, name: String) -> Unit,
    onNavigateToSettings: () -> Unit,
) {
    val state = viewModel.state.collectAsState()
    val searchQuery = remember { mutableStateOf("") }

    LaunchedEffect(Unit) {
        viewModel.load()
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
        ) {
            SearchBar(
                query = searchQuery.value,
                onQueryChange = { query ->
                    searchQuery.value = query
                    viewModel.filter(query)
                },
                onSearch = {},
                expanded = false,
                onExpandedChange = {},
                modifier = Modifier
                    .weight(1f)
                    .height(56.dp),
                placeholder = { Text("Search repos") },
            )
            Spacer(modifier = Modifier.width(8.dp))
            IconButton(onClick = onNavigateToSettings) {
                Icon(Icons.Default.Settings, contentDescription = "Settings")
            }
        }

        Spacer(modifier = Modifier.height(16.dp))

        when (val currentState = state.value) {
            RepoState.Loading -> {
                Text("Loading repositories...")
            }

            is RepoState.Success -> {
                LazyColumn {
                    items(currentState.repos) { repo ->
                        RepoItem(
                            repo = repo,
                            onClick = {
                                onRepoSelected(repo.owner, repo.name)
                            },
                        )
                    }
                }
            }

            is RepoState.Error -> {
                Text(
                    text = "Error: ${currentState.message}",
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}

@Composable
fun RepoItem(
    repo: Repo,
    onClick: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(12.dp),
    ) {
        Text(
            text = "${repo.owner}/${repo.name}",
            style = MaterialTheme.typography.titleMedium,
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = repo.description,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
