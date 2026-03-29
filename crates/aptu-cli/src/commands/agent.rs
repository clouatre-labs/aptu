// SPDX-License-Identifier: Apache-2.0

//! Agent orchestration command handler.

use crate::cli::{AgentCommand, OutputContext};

/// Run the agent command.
pub async fn run_agent_command(ctx: &OutputContext, command: AgentCommand) -> anyhow::Result<()> {
    match command {
        AgentCommand::Run {
            issue_ref,
            phase,
            handoff_dir,
            dry_run,
        } => {
            tracing::info!(
                issue_ref = %issue_ref,
                phase = ?phase,
                handoff_dir = %handoff_dir,
                dry_run = dry_run,
                "agent run invoked"
            );
            println!("agent run: {issue_ref} (not yet implemented)");
            let _ = (phase, handoff_dir, dry_run, ctx);
            Ok(())
        }
    }
}
