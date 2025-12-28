// SPDX-License-Identifier: Apache-2.0

//! Output formatting for release notes.

use std::io::{self, Write};

use console::style;
use serde::Serialize;

use crate::cli::OutputContext;
use crate::commands::release::ReleaseNotesOutput;
use crate::output::Renderable;

impl Serialize for ReleaseNotesOutput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ReleaseNotesOutput", 9)?;
        state.serialize_field("theme", &self.response.theme)?;
        state.serialize_field("narrative", &self.response.narrative)?;
        state.serialize_field("highlights", &self.response.highlights)?;
        state.serialize_field("features", &self.response.features)?;
        state.serialize_field("fixes", &self.response.fixes)?;
        state.serialize_field("improvements", &self.response.improvements)?;
        state.serialize_field("documentation", &self.response.documentation)?;
        state.serialize_field("maintenance", &self.response.maintenance)?;
        state.serialize_field("contributors", &self.response.contributors)?;
        state.end()
    }
}

/// Helper function to render sections with optional styling.
fn render_sections<F>(
    w: &mut dyn Write,
    response: &aptu_core::ReleaseNotesResponse,
    style_fn: F,
) -> io::Result<()>
where
    F: Fn(&str) -> String,
{
    writeln!(w)?;
    writeln!(w, "{}", style_fn(&format!("## {}", response.theme)))?;
    writeln!(w)?;
    writeln!(w, "{}", response.narrative)?;
    writeln!(w)?;

    if !response.highlights.is_empty() {
        writeln!(w, "{}", style_fn("### Highlights"))?;
        for highlight in &response.highlights {
            writeln!(w, "- {highlight}")?;
        }
        writeln!(w)?;
    }

    writeln!(w, "{}", style_fn("---"))?;
    writeln!(w)?;

    writeln!(w, "{}", style_fn("## Installation"))?;
    writeln!(
        w,
        "See the official documentation for installation instructions."
    )?;
    writeln!(w)?;

    writeln!(w, "{}", style_fn("---"))?;
    writeln!(w)?;

    writeln!(w, "{}", style_fn("## What's Changed"))?;

    if !response.features.is_empty() {
        writeln!(w, "{}", style_fn("### Features"))?;
        for feature in &response.features {
            writeln!(w, "- {feature}")?;
        }
        writeln!(w)?;
    }

    if !response.fixes.is_empty() {
        writeln!(w, "{}", style_fn("### Fixes"))?;
        for fix in &response.fixes {
            writeln!(w, "- {fix}")?;
        }
        writeln!(w)?;
    }

    if !response.improvements.is_empty() {
        writeln!(w, "{}", style_fn("### Improvements"))?;
        for improvement in &response.improvements {
            writeln!(w, "- {improvement}")?;
        }
        writeln!(w)?;
    }

    if !response.documentation.is_empty() {
        writeln!(w, "{}", style_fn("### Documentation"))?;
        for doc in &response.documentation {
            writeln!(w, "- {doc}")?;
        }
        writeln!(w)?;
    }

    if !response.maintenance.is_empty() {
        writeln!(w, "{}", style_fn("### Maintenance"))?;
        for maint in &response.maintenance {
            writeln!(w, "- {maint}")?;
        }
        writeln!(w)?;
    }

    writeln!(w, "{}", style_fn("---"))?;
    writeln!(w)?;

    writeln!(w, "{}", style_fn("## Contributors"))?;
    for contributor in &response.contributors {
        writeln!(w, "- {contributor}")?;
    }
    writeln!(w)?;

    Ok(())
}

impl Renderable for ReleaseNotesOutput {
    fn render_text(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        render_sections(w, &self.response, |s| format!("{}", style(s).bold().dim()))?;

        if self.dry_run {
            writeln!(w, "{}", style("(dry run - not posted)").yellow())?;
        }

        Ok(())
    }

    fn render_markdown(&self, w: &mut dyn Write, _ctx: &OutputContext) -> io::Result<()> {
        render_sections(w, &self.response, std::string::ToString::to_string)?;
        Ok(())
    }
}
