// SPDX-License-Identifier: Apache-2.0

//! TTL-based file caching for GitHub API responses.
//!
//! Stores issue and repository data as JSON files with embedded metadata
//! (timestamp, optional etag). Cache entries are validated against TTL settings
//! from configuration.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// A cached entry with metadata.
///
/// Wraps cached data with timestamp and optional etag for validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    /// The cached data.
    pub data: T,
    /// When the entry was cached.
    pub cached_at: DateTime<Utc>,
    /// Optional `ETag` for future conditional requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
}

impl<T> CacheEntry<T> {
    /// Create a new cache entry.
    pub fn new(data: T) -> Self {
        Self {
            data,
            cached_at: Utc::now(),
            etag: None,
        }
    }

    /// Create a new cache entry with an etag.
    pub fn with_etag(data: T, etag: String) -> Self {
        Self {
            data,
            cached_at: Utc::now(),
            etag: Some(etag),
        }
    }

    /// Check if this entry is still valid based on TTL.
    ///
    /// # Arguments
    ///
    /// * `ttl` - Time-to-live duration
    ///
    /// # Returns
    ///
    /// `true` if the entry is within its TTL, `false` if expired.
    pub fn is_valid(&self, ttl: Duration) -> bool {
        let now = Utc::now();
        now.signed_duration_since(self.cached_at) < ttl
    }
}

/// Returns the cache directory.
///
/// - Linux: `~/.cache/aptu`
/// - macOS: `~/Library/Caches/aptu`
/// - Windows: `C:\Users\<User>\AppData\Local\aptu`
#[must_use]
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .expect("Failed to determine cache directory")
        .join("aptu")
}

/// Generate a cache key for an issue list.
///
/// # Arguments
///
/// * `owner` - Repository owner
/// * `repo` - Repository name
///
/// # Returns
///
/// Cache key in format: `issues/{owner}_{repo}.json`
/// Generates a cache key for repository metadata (labels and milestones).
///
/// # Arguments
///
/// * `owner` - Repository owner
/// * `repo` - Repository name
///
/// # Returns
///
/// A cache key string in the format `repo_metadata/{owner}_{repo}.json`
#[must_use]
pub fn cache_key_repo_metadata(owner: &str, repo: &str) -> String {
    format!("repo_metadata/{owner}_{repo}.json")
}

/// A cache key string in the format `issues/{owner}_{repo}.json`
#[must_use]
pub fn cache_key_issues(owner: &str, repo: &str) -> String {
    format!("issues/{owner}_{repo}.json")
}

/// Generate a cache key for model lists.
///
/// # Arguments
///
/// * `provider` - Provider name (e.g., "openrouter", "gemini")
///
/// # Returns
///
/// A cache key string in the format `models/{provider}.json`
#[must_use]
pub fn cache_key_models(provider: &str) -> String {
    format!("models/{provider}.json")
}

/// Read a cache entry from disk.
///
/// # Arguments
///
/// * `key` - Cache key (relative path within cache directory)
///
/// # Returns
///
/// The deserialized cache entry, or `None` if the file doesn't exist.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read or parsed.
pub fn read_cache<T: for<'de> Deserialize<'de>>(key: &str) -> Result<Option<CacheEntry<T>>> {
    let path = cache_dir().join(key);

    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read cache file: {}", path.display()))?;

    let entry: CacheEntry<T> = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse cache file: {}", path.display()))?;

    Ok(Some(entry))
}

/// Write a cache entry to disk.
///
/// Creates parent directories if they don't exist.
/// Uses atomic write pattern (write to temp, rename) to prevent corruption.
///
/// # Arguments
///
/// * `key` - Cache key (relative path within cache directory)
/// * `entry` - Cache entry to write
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn write_cache<T: Serialize>(key: &str, entry: &CacheEntry<T>) -> Result<()> {
    let path = cache_dir().join(key);

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache directory: {}", parent.display()))?;
    }

    let contents =
        serde_json::to_string_pretty(entry).context("Failed to serialize cache entry")?;

    // Atomic write: write to temp file, then rename
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, contents)
        .with_context(|| format!("Failed to write cache temp file: {}", temp_path.display()))?;

    fs::rename(&temp_path, &path)
        .with_context(|| format!("Failed to rename cache file: {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        value: String,
        count: u32,
    }

    #[test]
    fn test_cache_entry_new() {
        let data = TestData {
            value: "test".to_string(),
            count: 42,
        };
        let entry = CacheEntry::new(data.clone());

        assert_eq!(entry.data, data);
        assert!(entry.etag.is_none());
    }

    #[test]
    fn test_cache_entry_with_etag() {
        let data = TestData {
            value: "test".to_string(),
            count: 42,
        };
        let etag = "abc123".to_string();
        let entry = CacheEntry::with_etag(data.clone(), etag.clone());

        assert_eq!(entry.data, data);
        assert_eq!(entry.etag, Some(etag));
    }

    #[test]
    fn test_cache_entry_is_valid_within_ttl() {
        let data = TestData {
            value: "test".to_string(),
            count: 42,
        };
        let entry = CacheEntry::new(data);
        let ttl = Duration::hours(1);

        assert!(entry.is_valid(ttl));
    }

    #[test]
    fn test_cache_entry_is_valid_expired() {
        let data = TestData {
            value: "test".to_string(),
            count: 42,
        };
        let mut entry = CacheEntry::new(data);
        // Manually set cached_at to 2 hours ago
        entry.cached_at = Utc::now() - Duration::hours(2);
        let ttl = Duration::hours(1);

        assert!(!entry.is_valid(ttl));
    }

    #[test]
    fn test_cache_key_issues() {
        let key = cache_key_issues("owner", "repo");
        assert_eq!(key, "issues/owner_repo.json");
    }

    #[test]
    fn test_cache_dir_path() {
        let dir = cache_dir();
        assert!(dir.ends_with("aptu"));
    }

    #[test]
    fn test_cache_serialization_with_etag() {
        let data = TestData {
            value: "test".to_string(),
            count: 42,
        };
        let etag = "xyz789".to_string();
        let entry = CacheEntry::with_etag(data.clone(), etag.clone());

        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: CacheEntry<TestData> = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.data, data);
        assert_eq!(parsed.etag, Some(etag));
    }

    #[test]
    fn test_read_cache_nonexistent() {
        let result: Result<Option<CacheEntry<TestData>>> = read_cache("nonexistent/file.json");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_write_and_read_cache() {
        let data = TestData {
            value: "cached".to_string(),
            count: 99,
        };
        let entry = CacheEntry::new(data.clone());
        let key = "test/data.json";

        // Write cache
        write_cache(key, &entry).expect("write cache");

        // Read cache
        let read_entry: CacheEntry<TestData> =
            read_cache(key).expect("read cache").expect("cache exists");

        assert_eq!(read_entry.data, data);
        assert_eq!(read_entry.etag, entry.etag);

        // Cleanup
        let path = cache_dir().join(key);
        if path.exists() {
            fs::remove_file(path).ok();
        }
    }
}
