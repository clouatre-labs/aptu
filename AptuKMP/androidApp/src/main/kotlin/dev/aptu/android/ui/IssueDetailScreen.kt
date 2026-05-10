// SPDX-License-Identifier: Apache-2.0

package dev.aptu.android.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun IssueDetailScreen(
    issueId: String,
    onNavigateBack: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp)
            .verticalScroll(rememberScrollState()),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
        ) {
            IconButton(onClick = onNavigateBack) {
                Icon(Icons.Default.ArrowBack, contentDescription = "Back")
            }
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                text = "Issue Details",
                style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.weight(1f),
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        Text(
            text = "Issue #$issueId",
            style = MaterialTheme.typography.headlineSmall,
        )

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = "Issue Title",
            style = MaterialTheme.typography.titleMedium,
        )

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = "Issue body content goes here. This is a placeholder for the full issue description.",
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.selectable(selected = true, onClick = {}),
        )

        Spacer(modifier = Modifier.height(16.dp))

        Text(
            text = "Labels",
            style = MaterialTheme.typography.titleSmall,
        )

        Spacer(modifier = Modifier.height(8.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
        ) {
            AssistChip(
                onClick = {},
                label = { Text("bug") },
            )
            Spacer(modifier = Modifier.width(8.dp))
            AssistChip(
                onClick = {},
                label = { Text("enhancement") },
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        Text(
            text = "Author: John Doe",
            style = MaterialTheme.typography.bodySmall,
        )

        Spacer(modifier = Modifier.height(4.dp))

        Text(
            text = "Created: 2024-01-15",
            style = MaterialTheme.typography.bodySmall,
        )

        Spacer(modifier = Modifier.height(24.dp))

        Button(
            onClick = {
                // TODO: Implement triage action
            },
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Triage Issue")
        }
    }
}
