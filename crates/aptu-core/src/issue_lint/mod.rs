// SPDX-License-Identifier: Apache-2.0

//! Deterministic, no-AI issue-body linting.
//!
//! Validates a GitHub issue body against template H2 headings and
//! readiness checks, mirroring the `security` module's local-only pattern.

pub mod parser;
pub mod specs;
pub mod types;

pub use parser::{
    extract_h2_headings, has_code_example, has_external_reference, strip_fenced_blocks,
    strip_html_comments,
};
pub use specs::{
    SPECS_FILE_NAME, SpecResolution, SpecsFile, find_spec, lint_issue, load_specs, parse_specs,
    resolve_specs, resolve_specs_in,
};
pub use types::{IssueLintResult, IssueLintSpec, IssueLintViolation};
