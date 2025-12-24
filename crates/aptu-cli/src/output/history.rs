// SPDX-License-Identifier: Apache-2.0

use aptu_core::history::ContributionStatus;
use aptu_core::utils::{format_relative_time, truncate};
use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::types::HistoryResult;

use super::Renderable;

impl Renderable for HistoryResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        if self.contributions.is_empty() {
            writeln!(w)?;
            writeln!(w, "{}", style("No contributions yet.").yellow())?;
            writeln!(w, "Run `aptu triage <url>` to get started!")?;
            writeln!(w)?;
            return Ok(());
        }

        writeln!(w)?;
        writeln!(
            w,
            "{}",
            style(format!(
                "Contribution history ({} total):",
                self.contributions.len()
            ))
            .bold()
        )?;
        writeln!(w)?;

        // Table header
        writeln!(
            w,
            "  {:<25} {:<8} {:<10} {:<15} {}",
            style("Repository").cyan(),
            style("Issue").cyan(),
            style("Action").cyan(),
            style("When").cyan(),
            style("Status").cyan()
        )?;
        writeln!(w, "  {}", style("-".repeat(75)).dim())?;

        for contribution in &self.contributions {
            let repo = truncate(&contribution.repo, 25);
            let issue = format!("#{}", contribution.issue);
            let when = format_relative_time(&contribution.timestamp);
            let status = match contribution.status {
                ContributionStatus::Pending => style("pending").yellow().to_string(),
                ContributionStatus::Accepted => style("accepted").green().to_string(),
                ContributionStatus::Rejected => style("rejected").red().to_string(),
            };
            writeln!(
                w,
                "  {:<25} {:<8} {:<10} {:<15} {}",
                repo,
                style(issue).green(),
                contribution.action,
                style(when).dim(),
                status
            )?;
        }

        // AI stats
        let total_tokens = self.history_data.total_tokens();
        let total_cost = self.history_data.total_cost();
        let avg_tokens = self.history_data.avg_tokens_per_triage();

        if total_tokens > 0 {
            writeln!(w)?;
            writeln!(w, "  {}", style("AI Usage Summary").cyan().bold())?;
            writeln!(w, "  {}", style("-".repeat(75)).dim())?;
            writeln!(
                w,
                "  Total tokens: {}",
                style(total_tokens.to_string()).green()
            )?;
            writeln!(
                w,
                "  Total cost: {}",
                style(format!("${total_cost:.4}")).green()
            )?;
            writeln!(
                w,
                "  Average tokens per triage: {}",
                style(format!("{avg_tokens:.0}")).green()
            )?;
        }
        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        if self.contributions.is_empty() {
            writeln!(w, "No contributions yet.")?;
            return Ok(());
        }

        writeln!(
            w,
            "## Contribution History ({} total)\n",
            self.contributions.len()
        )?;
        writeln!(w, "| Repository | Issue | Action | When | Status |")?;
        writeln!(w, "|------------|-------|--------|------|--------|")?;

        for contribution in &self.contributions {
            let repo = truncate(&contribution.repo, 25);
            let issue = format!("#{}", contribution.issue);
            let when = format_relative_time(&contribution.timestamp);
            let status = match contribution.status {
                ContributionStatus::Pending => "pending",
                ContributionStatus::Accepted => "accepted",
                ContributionStatus::Rejected => "rejected",
            };
            writeln!(
                w,
                "| {repo} | {issue} | {} | {when} | {status} |",
                contribution.action
            )?;
        }

        // AI stats
        let total_tokens = self.history_data.total_tokens();
        let total_cost = self.history_data.total_cost();
        let avg_tokens = self.history_data.avg_tokens_per_triage();

        if total_tokens > 0 {
            writeln!(w)?;
            writeln!(w, "### AI Usage Summary")?;
            writeln!(w)?;
            writeln!(w, "- Total tokens: {total_tokens}")?;
            writeln!(w, "- Total cost: ${total_cost:.4}")?;
            writeln!(w, "- Average tokens per triage: {avg_tokens:.0}")?;
        }
        Ok(())
    }
}
