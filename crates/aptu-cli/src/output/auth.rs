// SPDX-License-Identifier: Apache-2.0

use console::style;
use std::io::{self, Write};

use crate::cli::OutputContext;
use crate::commands::types::AuthStatusResult;

use super::Renderable;

impl Renderable for AuthStatusResult {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w)?;
        if self.authenticated {
            writeln!(w, "{} Authenticated with GitHub", style("*").green().bold())?;
            if let Some(ref method) = self.method {
                writeln!(w, "  Method: {}", style(method.to_string()).cyan())?;
            }
            if let Some(ref username) = self.username {
                writeln!(w, "  Username: {}", style(username).cyan())?;
            }
        } else {
            writeln!(
                w,
                "{} Not authenticated. Run {} to authenticate.",
                style("!").yellow().bold(),
                style("aptu auth login").cyan()
            )?;
        }
        writeln!(w)?;
        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        writeln!(w, "## Authentication Status\n")?;
        if self.authenticated {
            writeln!(w, "**Status:** Authenticated")?;
            if let Some(ref method) = self.method {
                writeln!(w, "**Method:** {method}")?;
            }
            if let Some(ref username) = self.username {
                writeln!(w, "**Username:** {username}")?;
            }
        } else {
            writeln!(w, "**Status:** Not authenticated")?;
        }
        Ok(())
    }
}
