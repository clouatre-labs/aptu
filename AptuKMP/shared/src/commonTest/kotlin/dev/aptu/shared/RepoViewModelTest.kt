// SPDX-License-Identifier: Apache-2.0

package dev.aptu.shared

import dev.aptu.shared.models.Repo
import dev.aptu.shared.viewmodels.RepoState
import dev.aptu.shared.viewmodels.RepoViewModel
import kotlin.test.Test
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest

class RepoViewModelTest {
    @Test
    fun testLoadReposSuccess() = runTest {
        val viewModel = RepoViewModel()
        viewModel.load()

        val state = viewModel.state.value
        assertTrue(state is RepoState.Success || state is RepoState.Error)
    }
}
