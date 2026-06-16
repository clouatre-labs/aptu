// SPDX-License-Identifier: Apache-2.0

//! Dependency release note enrichment from registry APIs.
//!
//! Detects version bumps in Cargo.toml, package.json, and pyproject.toml PR diffs,
//! resolves upstream GitHub URLs via registry APIs, and fetches release notes via Octocrab.
//! All operations are soft-fail: errors are recorded in `fetch_note` and never block review.

#![allow(
    clippy::doc_markdown,
    clippy::manual_let_else,
    clippy::needless_continue,
    clippy::single_match_else
)]

use crate::ai::types::DepReleaseNote;
use futures::future::join_all;
use regex::Regex;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

// Module-level regex statics to avoid recompilation on every call
static CARGO_VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"version\s*=\s*"([^"]+)""#).expect("valid regex"));

static CARGO_NAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"name\s*=\s*"([^"]+)""#).expect("valid regex"));

static NPM_VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""version"\s*:\s*"([^"]+)""#).expect("valid regex"));

static NPM_NAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""name"\s*:\s*"([^"]+)""#).expect("valid regex"));

static PYPI_VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"version\s*=\s*"?([^"]+)"?"#).expect("valid regex"));

static PYPI_NAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"name\s*=\s*"([^"]+)""#).expect("valid regex"));

/// Detects version bumps in a manifest file diff.
/// Returns `(package_name, old_version, new_version)` tuples.
///
/// Uses a two-phase approach:
/// - Phase 1: Scan ALL lines (including context lines) to collect name candidates with their line indices.
/// - Phase 2: Find version bumps in added/removed lines and attribute the nearest preceding name.
///
/// # Limitations
/// Regex-based parsing works for the common single-line forms (`name = "pkg"`, `"name": "pkg"`).
/// Multi-line version definitions (e.g. TOML inline tables split across lines, or workspace
/// `{ workspace = true }` entries) and complex structures may yield `"unknown"` as the package
/// name; those entries are skipped rather than triggering a spurious registry fetch.
fn detect_version_bumps(
    filename: &str,
    patch: &str,
    manifest_name: &str,
    version_regex: &Regex,
    name_regex: &Regex,
) -> Vec<(String, String, String)> {
    if !filename.ends_with(manifest_name) {
        return Vec::new();
    }

    let patch_lines: Vec<&str> = patch.lines().collect();

    // Phase 1: Collect all name candidates from the entire patch (all lines, not just +/-)
    let mut name_candidates: Vec<(usize, String)> = Vec::new();
    for (line_idx, line) in patch_lines.iter().enumerate() {
        if let Some(caps) = name_regex.captures(line)
            && let Some(name) = caps.get(1).map(|m| m.as_str())
        {
            name_candidates.push((line_idx, name.to_string()));
        }
    }

    // Phase 2: Find version bumps and attribute nearest preceding name
    let mut removed_lines = Vec::new();
    let mut added_lines = Vec::new();

    for (line_idx, line) in patch_lines.iter().enumerate() {
        if line.starts_with('-') && !line.starts_with("---") {
            let content = &line[1..];
            removed_lines.push((line_idx, content.to_string()));
        } else if line.starts_with('+') && !line.starts_with("+++") {
            let content = &line[1..];
            added_lines.push((line_idx, content.to_string()));
        }
    }

    let mut bumps = Vec::new();
    for (removed_idx, removed) in &removed_lines {
        if let Some(caps) = version_regex.captures(removed)
            && let Some(old_version) = caps.get(1).map(|m| m.as_str())
        {
            for (_, added) in &added_lines {
                if let Some(caps) = version_regex.captures(added)
                    && let Some(new_version) = caps.get(1).map(|m| m.as_str())
                    && old_version != new_version
                {
                    // Find the nearest name candidate with line_index <= removed_idx
                    let package_name = name_candidates
                        .iter()
                        .rfind(|(idx, _)| *idx <= *removed_idx)
                        .map_or_else(|| "unknown".to_string(), |(_, name)| name.clone());

                    bumps.push((
                        package_name,
                        old_version.to_string(),
                        new_version.to_string(),
                    ));
                }
            }
        }
    }
    bumps
}

/// Resolves GitHub URL for a package via registry API.
/// Returns (registry_name, github_url, fetch_note).
async fn resolve_github_url(
    client: &reqwest::Client,
    package_name: &str,
    registry: &str,
) -> (String, Option<String>, String) {
    let (url, json_path) = match registry {
        "crates.io" => (
            format!("https://crates.io/api/v1/crates/{package_name}"),
            vec!["crate", "repository"],
        ),
        "npm" => (
            format!("https://registry.npmjs.org/{package_name}"),
            vec!["repository", "url"],
        ),
        "pypi" => (
            format!("https://pypi.org/pypi/{package_name}/json"),
            vec!["info", "home_page"],
        ),
        _ => return (registry.to_string(), None, "Unknown registry".to_string()),
    };

    let resp = match client
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return (
                registry.to_string(),
                None,
                "Registry API timeout".to_string(),
            );
        }
    };

    let json = match resp.json::<serde_json::Value>().await {
        Ok(j) => j,
        Err(_) => {
            return (
                registry.to_string(),
                None,
                "Invalid registry response".to_string(),
            );
        }
    };

    let mut repo_url = None;
    let mut current = &json;
    for key in &json_path {
        if let Some(next) = current.get(key) {
            current = next;
        } else {
            break;
        }
    }

    if let Some(url_str) = current.as_str() {
        let clean_url = url_str
            .strip_prefix("git+")
            .unwrap_or(url_str)
            .strip_suffix(".git")
            .unwrap_or(url_str);
        if clean_url.contains("github.com") {
            repo_url = Some(clean_url.to_string());
        } else {
            return (
                registry.to_string(),
                None,
                format!("Non-GitHub URL filtered: {clean_url}"),
            );
        }
    }

    match repo_url {
        Some(url) => (registry.to_string(), Some(url), String::new()),
        None => (
            registry.to_string(),
            None,
            "No repository URL in registry response".to_string(),
        ),
    }
}

/// Fetches release notes from GitHub via Octocrab.
#[cfg(not(target_arch = "wasm32"))]
async fn release_notes_from_octocrab(
    owner: &str,
    repo: &str,
    new_version: &str,
    max_chars: usize,
) -> (String, String) {
    let token = match std::env::var("GITHUB_TOKEN") {
        Ok(t) => t,
        Err(_) => return (String::new(), "GITHUB_TOKEN not set".to_string()),
    };

    let octocrab = match octocrab::OctocrabBuilder::new()
        .personal_token(secrecy::SecretString::new(token.into()))
        .build()
    {
        Ok(o) => o,
        Err(_) => return (String::new(), "Failed to initialize Octocrab".to_string()),
    };

    for tag in &[format!("v{new_version}"), new_version.to_string()] {
        match octocrab.repos(owner, repo).releases().get_by_tag(tag).await {
            Ok(release) => {
                let body = release.body.unwrap_or_default();
                let truncated = if body.len() > max_chars {
                    body[..max_chars].to_string()
                } else {
                    body
                };
                return (truncated, String::new());
            }
            Err(_) => continue,
        }
    }

    (String::new(), "Release tag not found".to_string())
}

/// Parses GitHub URL to extract owner and repo.
fn parse_github_url(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches(".git");
    let parts: Vec<&str> = url.split('/').collect();
    if parts.len() >= 2 {
        let repo = parts[parts.len() - 1].to_string();
        let owner = parts[parts.len() - 2].to_string();
        return Some((owner, repo));
    }
    None
}

/// Enriches a single package with release notes.
/// Helper function for parallel processing.
async fn enrich_single_package(
    client: Arc<reqwest::Client>,
    package_name: String,
    old_version: String,
    new_version: String,
    registry: &str,
    max_chars: usize,
) -> DepReleaseNote {
    let (registry_name, github_url_opt, mut fetch_note) =
        resolve_github_url(&client, &package_name, registry).await;

    let github_url = match github_url_opt {
        Some(url) => url,
        None => {
            return DepReleaseNote {
                package_name,
                old_version,
                new_version,
                registry: registry_name,
                github_url: String::new(),
                body: String::new(),
                fetch_note,
            };
        }
    };

    let Some((owner, repo)) = parse_github_url(&github_url) else {
        return DepReleaseNote {
            package_name,
            old_version,
            new_version,
            registry: registry_name,
            github_url,
            body: String::new(),
            fetch_note: "Invalid GitHub URL".to_string(),
        };
    };

    #[cfg(not(target_arch = "wasm32"))]
    let (body, release_fetch_note) =
        release_notes_from_octocrab(&owner, &repo, &new_version, max_chars).await;
    #[cfg(target_arch = "wasm32")]
    let (body, release_fetch_note) = (
        String::new(),
        "GitHub fetch unavailable on wasm32".to_string(),
    );

    if !release_fetch_note.is_empty() {
        fetch_note = release_fetch_note;
    }

    DepReleaseNote {
        package_name,
        old_version,
        new_version,
        registry: registry_name,
        github_url,
        body,
        fetch_note,
    }
}

/// Enriches PR with dependency release notes.
/// Returns a vector of `DepReleaseNote`; never fails (all errors recorded in `fetch_note`).
pub async fn enrich_dep_releases(
    pr_files: &[crate::ai::types::PrFile],
    max_packages: usize,
    max_chars: usize,
) -> Vec<DepReleaseNote> {
    // Create a single client for all registry API calls, wrapped in Arc for sharing across futures
    let client = Arc::new(reqwest::Client::new());

    // Collect all (package_name, old_version, new_version, registry) tuples to process
    let mut packages_to_enrich = Vec::new();

    for file in pr_files {
        if packages_to_enrich.len() >= max_packages {
            break;
        }

        if let Some(patch) = &file.patch {
            let (registry, version_regex, name_regex) = if file.filename.ends_with("Cargo.toml") {
                ("crates.io", &*CARGO_VERSION_REGEX, &*CARGO_NAME_REGEX)
            } else if file.filename.ends_with("package.json") {
                ("npm", &*NPM_VERSION_REGEX, &*NPM_NAME_REGEX)
            } else if file.filename.ends_with("pyproject.toml") {
                ("pypi", &*PYPI_VERSION_REGEX, &*PYPI_NAME_REGEX)
            } else {
                continue;
            };

            let bumps = detect_version_bumps(
                &file.filename,
                patch,
                &file.filename,
                version_regex,
                name_regex,
            );

            for (package_name, old_version, new_version) in bumps {
                if packages_to_enrich.len() >= max_packages {
                    break;
                }
                if package_name == "unknown" {
                    continue;
                }
                packages_to_enrich.push((package_name, old_version, new_version, registry));
            }
        }
    }

    // Create futures for all packages and resolve them in parallel
    let futures = packages_to_enrich
        .into_iter()
        .map(|(package_name, old_version, new_version, registry)| {
            enrich_single_package(
                Arc::clone(&client),
                package_name,
                old_version,
                new_version,
                registry,
                max_chars,
            )
        })
        .collect::<Vec<_>>();

    join_all(futures).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cargo_version_bump() {
        let patch = "-version = \"1.0.0\"\n+version = \"2.0.0\"";
        let version_regex = Regex::new(r#"version\s*=\s*"([^"]+)""#).unwrap();
        let name_regex = Regex::new(r#"name\s*=\s*"([^"]+)""#).unwrap();
        let bumps = detect_version_bumps(
            "Cargo.toml",
            patch,
            "Cargo.toml",
            &version_regex,
            &name_regex,
        );
        assert!(!bumps.is_empty());
        assert_eq!(bumps[0].1, "1.0.0");
        assert_eq!(bumps[0].2, "2.0.0");
    }

    #[test]
    fn test_parse_github_url() {
        let url = "https://github.com/tokio-rs/tokio";
        let (owner, repo) = parse_github_url(url).unwrap();
        assert_eq!(owner, "tokio-rs");
        assert_eq!(repo, "tokio");
    }
}
