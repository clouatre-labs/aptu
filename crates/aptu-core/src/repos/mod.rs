// SPDX-License-Identifier: Apache-2.0

//! Curated repository list for Aptu.
//!
//! This module provides a hardcoded list of repositories known to be:
//! - Active (commits in last 30 days)
//! - Welcoming (good first issue labels exist)
//! - Responsive (maintainers reply within 1 week)

use serde::Serialize;
use tracing::debug;

/// A curated repository for contribution.
#[derive(Debug, Clone, Serialize)]
pub struct CuratedRepo {
    /// Repository owner (user or organization).
    pub owner: &'static str,
    /// Repository name.
    pub name: &'static str,
    /// Primary programming language.
    pub language: &'static str,
    /// Short description.
    pub description: &'static str,
}

impl CuratedRepo {
    /// Returns the full repository name in "owner/name" format.
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// Hardcoded list of curated repositories.
const CURATED_REPOS: &[CuratedRepo] = &[
    CuratedRepo {
        owner: "block",
        name: "goose",
        language: "Rust",
        description: "AI developer agent",
    },
    CuratedRepo {
        owner: "astral-sh",
        name: "ruff",
        language: "Rust",
        description: "Fast Python linter",
    },
    CuratedRepo {
        owner: "astral-sh",
        name: "uv",
        language: "Rust",
        description: "Fast Python package manager",
    },
    CuratedRepo {
        owner: "tauri-apps",
        name: "tauri",
        language: "Rust",
        description: "Desktop app framework",
    },
    CuratedRepo {
        owner: "bevyengine",
        name: "bevy",
        language: "Rust",
        description: "Game engine",
    },
    CuratedRepo {
        owner: "lapce",
        name: "lapce",
        language: "Rust",
        description: "Code editor",
    },
    CuratedRepo {
        owner: "zed-industries",
        name: "zed",
        language: "Rust",
        description: "Code editor",
    },
    CuratedRepo {
        owner: "eza-community",
        name: "eza",
        language: "Rust",
        description: "Modern ls replacement",
    },
    CuratedRepo {
        owner: "sharkdp",
        name: "bat",
        language: "Rust",
        description: "Cat clone with syntax highlighting",
    },
    CuratedRepo {
        owner: "BurntSushi",
        name: "ripgrep",
        language: "Rust",
        description: "Fast grep replacement",
    },
];

/// Returns the list of curated repositories.
pub fn list() -> &'static [CuratedRepo] {
    debug!("Listing {} curated repositories", CURATED_REPOS.len());
    CURATED_REPOS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_returns_non_empty() {
        let repos = list();
        assert!(!repos.is_empty(), "Curated repos list should not be empty");
        assert_eq!(repos.len(), 10, "Expected 10 curated repos");
    }

    #[test]
    fn full_name_format() {
        let repo = &list()[0];
        assert_eq!(repo.full_name(), "block/goose");
    }
}
