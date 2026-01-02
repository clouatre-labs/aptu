// SPDX-License-Identifier: Apache-2.0

//! Custom repository management.
//!
//! Provides functionality to read, write, and validate custom repositories
//! stored in TOML format at `~/.config/aptu/repos.toml`.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::config::config_dir;
use crate::error::AptuError;
use crate::repos::CuratedRepo;

/// Returns the path to the custom repositories file.
#[must_use]
pub fn repos_file_path() -> PathBuf {
    config_dir().join("repos.toml")
}

/// Custom repositories file structure.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CustomReposFile {
    /// Map of repository full names to repository data.
    #[serde(default)]
    pub repos: HashMap<String, CustomRepoEntry>,
}

/// A custom repository entry in the TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRepoEntry {
    /// Repository owner.
    pub owner: String,
    /// Repository name.
    pub name: String,
    /// Primary programming language.
    pub language: String,
    /// Short description.
    pub description: String,
}

impl From<CustomRepoEntry> for CuratedRepo {
    fn from(entry: CustomRepoEntry) -> Self {
        CuratedRepo {
            owner: entry.owner,
            name: entry.name,
            language: entry.language,
            description: entry.description,
        }
    }
}

/// Read custom repositories from TOML file.
///
/// Returns an empty vector if the file does not exist.
///
/// # Errors
///
/// Returns an error if the file exists but is invalid TOML.
#[instrument]
pub fn read_custom_repos() -> crate::Result<Vec<CuratedRepo>> {
    let path = repos_file_path();

    if !path.exists() {
        debug!("Custom repos file does not exist: {:?}", path);
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&path).map_err(|e| AptuError::Config {
        message: format!("Failed to read custom repos file: {e}"),
    })?;

    let file: CustomReposFile = toml::from_str(&content).map_err(|e| AptuError::Config {
        message: format!("Failed to parse custom repos TOML: {e}"),
    })?;

    let repos: Vec<CuratedRepo> = file.repos.into_values().map(CuratedRepo::from).collect();

    debug!("Read {} custom repositories", repos.len());
    Ok(repos)
}

/// Write custom repositories to TOML file.
///
/// Creates the config directory if it does not exist.
///
/// # Errors
///
/// Returns an error if the file cannot be written.
#[instrument(skip(repos))]
pub fn write_custom_repos(repos: &[CuratedRepo]) -> crate::Result<()> {
    let path = repos_file_path();

    // Ensure config directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AptuError::Config {
            message: format!("Failed to create config directory: {e}"),
        })?;
    }

    let mut file = CustomReposFile::default();
    for repo in repos {
        file.repos.insert(
            repo.full_name(),
            CustomRepoEntry {
                owner: repo.owner.clone(),
                name: repo.name.clone(),
                language: repo.language.clone(),
                description: repo.description.clone(),
            },
        );
    }

    let content = toml::to_string_pretty(&file).map_err(|e| AptuError::Config {
        message: format!("Failed to serialize custom repos: {e}"),
    })?;

    fs::write(&path, content).map_err(|e| AptuError::Config {
        message: format!("Failed to write custom repos file: {e}"),
    })?;

    debug!("Wrote {} custom repositories", repos.len());
    Ok(())
}

/// Validate and fetch metadata for a repository via GitHub API.
///
/// Fetches repository metadata from GitHub to ensure the repository exists
/// and is accessible.
///
/// # Arguments
///
/// * `owner` - Repository owner
/// * `name` - Repository name
///
/// # Returns
///
/// A `CuratedRepo` with metadata fetched from GitHub.
///
/// # Errors
///
/// Returns an error if the repository cannot be found or accessed.
#[instrument]
pub async fn validate_and_fetch_metadata(owner: &str, name: &str) -> crate::Result<CuratedRepo> {
    use octocrab::Octocrab;

    let client = Octocrab::builder().build()?;
    let repo = client
        .repos(owner, name)
        .get()
        .await
        .map_err(|e| AptuError::GitHub {
            message: format!("Failed to fetch repository metadata: {e}"),
        })?;

    let language = repo
        .language
        .map_or_else(|| "Unknown".to_string(), |v| v.to_string());
    let description = repo.description.map_or_else(String::new, |v| v.clone());

    Ok(CuratedRepo {
        owner: owner.to_string(),
        name: name.to_string(),
        language,
        description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_repos_toml_roundtrip() {
        let repos = vec![
            CuratedRepo {
                owner: "test".to_string(),
                name: "repo1".to_string(),
                language: "Rust".to_string(),
                description: "Test repo 1".to_string(),
            },
            CuratedRepo {
                owner: "test".to_string(),
                name: "repo2".to_string(),
                language: "Python".to_string(),
                description: "Test repo 2".to_string(),
            },
        ];

        // Serialize to TOML
        let mut file = CustomReposFile::default();
        for repo in &repos {
            file.repos.insert(
                repo.full_name(),
                CustomRepoEntry {
                    owner: repo.owner.clone(),
                    name: repo.name.clone(),
                    language: repo.language.clone(),
                    description: repo.description.clone(),
                },
            );
        }

        let toml_str = toml::to_string_pretty(&file).expect("should serialize");

        // Deserialize from TOML
        let parsed: CustomReposFile = toml::from_str(&toml_str).expect("should deserialize");
        let result: Vec<CuratedRepo> = parsed.repos.into_values().map(CuratedRepo::from).collect();

        assert_eq!(result.len(), 2);
        let full_names: std::collections::HashSet<_> =
            result.iter().map(super::super::CuratedRepo::full_name).collect();
        assert!(full_names.contains("test/repo1"));
        assert!(full_names.contains("test/repo2"));
    }
}
