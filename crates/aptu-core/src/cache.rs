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

/// Default TTL for issue cache entries (in minutes).
pub const DEFAULT_ISSUE_TTL_MINS: i64 = 60;

/// Default TTL for repository cache entries (in hours).
pub const DEFAULT_REPO_TTL_HOURS: i64 = 24;

/// Default TTL for model registry cache entries (in seconds).
pub const DEFAULT_MODEL_TTL_SECS: u64 = 86400;

/// Default TTL for security finding cache entries (in days).
pub const DEFAULT_SECURITY_TTL_DAYS: i64 = 7;

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

/// Trait for TTL-based filesystem caching.
///
/// Provides a unified interface for caching serializable data with time-to-live validation.
pub trait FileCache<V> {
    /// Get a cached value if it exists and is valid.
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key (filename without extension)
    ///
    /// # Returns
    ///
    /// The cached value if it exists and is within TTL, `None` otherwise.
    fn get(&self, key: &str) -> Result<Option<V>>;

    /// Get a cached value regardless of TTL (stale fallback).
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key (filename without extension)
    ///
    /// # Returns
    ///
    /// The cached value if it exists, `None` otherwise.
    fn get_stale(&self, key: &str) -> Result<Option<V>>;

    /// Set a cached value.
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key (filename without extension)
    /// * `value` - Value to cache
    fn set(&self, key: &str, value: &V) -> Result<()>;

    /// Remove a cached value.
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key (filename without extension)
    fn remove(&self, key: &str) -> Result<()>;
}

/// File-based cache implementation with TTL support.
///
/// Stores serialized data in JSON files with embedded metadata.
pub struct FileCacheImpl<V> {
    cache_dir: PathBuf,
    ttl: Duration,
    subdirectory: String,
    _phantom: std::marker::PhantomData<V>,
}

impl<V> FileCacheImpl<V>
where
    V: Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new file cache with default cache directory.
    ///
    /// # Arguments
    ///
    /// * `subdirectory` - Subdirectory within cache directory
    /// * `ttl` - Time-to-live for cache entries
    #[must_use]
    pub fn new(subdirectory: impl Into<String>, ttl: Duration) -> Self {
        Self::with_dir(cache_dir(), subdirectory, ttl)
    }

    /// Create a new file cache with custom cache directory.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Base cache directory
    /// * `subdirectory` - Subdirectory within cache directory
    /// * `ttl` - Time-to-live for cache entries
    #[must_use]
    pub fn with_dir(cache_dir: PathBuf, subdirectory: impl Into<String>, ttl: Duration) -> Self {
        Self {
            cache_dir,
            ttl,
            subdirectory: subdirectory.into(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the full path for a cache key.
    ///
    /// # Panics
    ///
    /// Panics if the key contains path separators or parent directory references,
    /// which could lead to path traversal vulnerabilities.
    fn cache_path(&self, key: &str) -> PathBuf {
        // Validate key to prevent path traversal
        assert!(
            !key.contains('/') && !key.contains('\\') && !key.contains(".."),
            "cache key must not contain path separators or '..': {key}"
        );

        let filename = if std::path::Path::new(key)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            key.to_string()
        } else {
            format!("{key}.json")
        };
        self.cache_dir.join(&self.subdirectory).join(filename)
    }
}

impl<V> FileCache<V> for FileCacheImpl<V>
where
    V: Serialize + for<'de> Deserialize<'de>,
{
    fn get(&self, key: &str) -> Result<Option<V>> {
        let path = self.cache_path(key);

        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read cache file: {}", path.display()))?;

        let entry: CacheEntry<V> = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse cache file: {}", path.display()))?;

        if entry.is_valid(self.ttl) {
            Ok(Some(entry.data))
        } else {
            Ok(None)
        }
    }

    fn get_stale(&self, key: &str) -> Result<Option<V>> {
        let path = self.cache_path(key);

        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read cache file: {}", path.display()))?;

        let entry: CacheEntry<V> = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse cache file: {}", path.display()))?;

        Ok(Some(entry.data))
    }

    fn set(&self, key: &str, value: &V) -> Result<()> {
        let path = self.cache_path(key);

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create cache directory: {}", parent.display())
            })?;
        }

        let entry = CacheEntry::new(value);
        let contents =
            serde_json::to_string_pretty(&entry).context("Failed to serialize cache entry")?;

        // Atomic write: write to temp file, then rename
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, contents)
            .with_context(|| format!("Failed to write cache temp file: {}", temp_path.display()))?;

        fs::rename(&temp_path, &path)
            .with_context(|| format!("Failed to rename cache file: {}", path.display()))?;

        Ok(())
    }

    fn remove(&self, key: &str) -> Result<()> {
        let path = self.cache_path(key);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to remove cache file: {}", path.display()))?;
        }
        Ok(())
    }
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
    fn test_file_cache_get_set() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let data = TestData {
            value: "test".to_string(),
            count: 42,
        };

        // Set value
        cache.set("test_key", &data).expect("set cache");

        // Get value
        let result = cache.get("test_key").expect("get cache");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);

        // Cleanup
        cache.remove("test_key").ok();
    }

    #[test]
    fn test_file_cache_get_miss() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));

        let result = cache.get("nonexistent").expect("get cache");
        assert!(result.is_none());
    }

    #[test]
    fn test_file_cache_get_stale() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::seconds(0));
        let data = TestData {
            value: "stale".to_string(),
            count: 99,
        };

        // Set value
        cache.set("stale_key", &data).expect("set cache");

        // Wait for TTL to expire
        std::thread::sleep(std::time::Duration::from_millis(10));

        // get() should return None (expired)
        let result = cache.get("stale_key").expect("get cache");
        assert!(result.is_none());

        // get_stale() should return the value
        let stale_result = cache.get_stale("stale_key").expect("get stale cache");
        assert!(stale_result.is_some());
        assert_eq!(stale_result.unwrap(), data);

        // Cleanup
        cache.remove("stale_key").ok();
    }

    #[test]
    fn test_file_cache_remove() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let data = TestData {
            value: "remove_me".to_string(),
            count: 1,
        };

        // Set value
        cache.set("remove_key", &data).expect("set cache");

        // Verify it exists
        assert!(cache.get("remove_key").expect("get cache").is_some());

        // Remove it
        cache.remove("remove_key").expect("remove cache");

        // Verify it's gone
        assert!(cache.get("remove_key").expect("get cache").is_none());
    }

    #[test]
    #[should_panic(expected = "cache key must not contain path separators")]
    fn test_cache_key_rejects_forward_slash() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let _ = cache.get("../etc/passwd");
    }

    #[test]
    #[should_panic(expected = "cache key must not contain path separators")]
    fn test_cache_key_rejects_backslash() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let _ = cache.get("..\\windows\\system32");
    }

    #[test]
    #[should_panic(expected = "cache key must not contain path separators")]
    fn test_cache_key_rejects_parent_dir() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let _ = cache.get("foo..bar");
    }
}
