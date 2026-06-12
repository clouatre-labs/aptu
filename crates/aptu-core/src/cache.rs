// SPDX-License-Identifier: Apache-2.0

//! TTL-based file caching for GitHub API responses.
//!
//! Stores issue and repository data as JSON files with embedded metadata
//! (timestamp, optional etag). Cache entries are validated against TTL settings
//! from configuration.

// `async_yields_async` is suppressed because the FileCache trait uses async fn (RPITIT,
// stable in Rust 1.95 / edition 2024). The trait is intentionally crate-internal
// (not part of the public API) and is never used as `dyn FileCache`, so the lint
// warning is a false positive. There is no plan to expose this trait publicly.

use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Ensures the cache unavailable warning is only emitted once.
static CACHE_UNAVAILABLE_WARNING: OnceLock<()> = OnceLock::new();

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
///
/// Returns `None` if the cache directory cannot be determined.
#[must_use]
pub fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("aptu"))
}

/// Trait for TTL-based filesystem caching.
///
/// Provides a unified interface for caching serializable data with time-to-live validation.
///
/// `async_fn_in_trait` is suppressed because this trait is re-exported for use by crate
/// consumers but is never intended to be implemented externally or used as `dyn FileCache`.
/// All known implementors are in this crate, so auto-trait bounds are not a concern.
#[allow(async_fn_in_trait)]
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
    async fn get(&self, key: &str) -> Result<Option<V>>;

    /// Get a cached value regardless of TTL (stale fallback).
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key (filename without extension)
    ///
    /// # Returns
    ///
    /// The cached value if it exists, `None` otherwise.
    async fn get_stale(&self, key: &str) -> Result<Option<V>>;

    /// Set a cached value.
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key (filename without extension)
    /// * `value` - Value to cache
    async fn set(&self, key: &str, value: &V) -> Result<()>;

    /// Remove a cached value.
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key (filename without extension)
    async fn remove(&self, key: &str) -> Result<()>;
}

/// File-based cache implementation with TTL support.
///
/// Stores serialized data in JSON files with embedded metadata.
/// When cache directory is unavailable (None), all operations become no-ops.
pub struct FileCacheImpl<V> {
    cache_dir: Option<PathBuf>,
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
    ///
    /// If the cache directory cannot be determined, caching is disabled
    /// and a warning is emitted.
    #[must_use]
    pub fn new(subdirectory: impl Into<String>, ttl: Duration) -> Self {
        let cache_dir = cache_dir();
        if cache_dir.is_none() {
            CACHE_UNAVAILABLE_WARNING.get_or_init(|| {
                warn!("Cache directory unavailable, caching disabled");
            });
        }
        Self::with_dir(cache_dir, subdirectory, ttl)
    }

    /// Create a new file cache with custom cache directory.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Base cache directory (None to disable caching)
    /// * `subdirectory` - Subdirectory within cache directory
    /// * `ttl` - Time-to-live for cache entries
    #[must_use]
    pub fn with_dir(
        cache_dir: Option<PathBuf>,
        subdirectory: impl Into<String>,
        ttl: Duration,
    ) -> Self {
        Self {
            cache_dir,
            ttl,
            subdirectory: subdirectory.into(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Check if caching is enabled.
    fn is_enabled(&self) -> bool {
        self.cache_dir.is_some()
    }

    /// Get the full path for a cache key.
    ///
    /// # Panics
    ///
    /// Panics if the key contains path separators or parent directory references,
    /// which could lead to path traversal vulnerabilities.
    fn cache_path(&self, key: &str) -> Option<PathBuf> {
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
        self.cache_dir
            .as_ref()
            .map(|dir| dir.join(&self.subdirectory).join(filename))
    }

    /// Evict cache files older than the specified TTL.
    ///
    /// Scans the cache subdirectory and removes files with `cached_at` timestamps
    /// older than `eviction_days`. Returns the count of files removed.
    ///
    /// # Arguments
    ///
    /// * `eviction_days` - Number of days to retain files
    ///
    /// # Returns
    ///
    /// The number of files evicted.
    pub async fn evict_stale(&self, eviction_days: i64) -> usize {
        if !self.is_enabled() {
            return 0;
        }

        let Some(cache_dir) = &self.cache_dir else {
            return 0;
        };

        let subdir = cache_dir.join(&self.subdirectory);

        // Check if subdirectory exists
        if !tokio::fs::try_exists(&subdir).await.unwrap_or(false) {
            return 0;
        }

        let Ok(mut read_dir) = tokio::fs::read_dir(&subdir).await else {
            return 0;
        };

        let mut evicted_count = 0;
        let cutoff_time = Utc::now() - Duration::days(eviction_days);

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();

            // Only process .json files
            if !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                continue;
            }

            let Ok(contents) = tokio::fs::read_to_string(&path).await else {
                continue;
            };

            let Ok(entry_data) = serde_json::from_str::<CacheEntry<serde_json::Value>>(&contents)
            else {
                continue;
            };

            if entry_data.cached_at < cutoff_time && tokio::fs::remove_file(&path).await.is_ok() {
                debug!("Evicted stale cache file: {}", path.display());
                evicted_count += 1;
            }
        }

        evicted_count
    }
}

impl<V> FileCache<V> for FileCacheImpl<V>
where
    V: Serialize + for<'de> Deserialize<'de>,
{
    async fn get(&self, key: &str) -> Result<Option<V>> {
        if !self.is_enabled() {
            return Ok(None);
        }

        let Some(path) = self.cache_path(key) else {
            return Ok(None);
        };

        if !tokio::fs::try_exists(&path)
            .await
            .with_context(|| format!("Failed to check cache file: {}", path.display()))?
        {
            return Ok(None);
        }

        let contents = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read cache file: {}", path.display()))?;

        let entry: CacheEntry<V> = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse cache file: {}", path.display()))?;

        if entry.is_valid(self.ttl) {
            Ok(Some(entry.data))
        } else {
            Ok(None)
        }
    }

    async fn get_stale(&self, key: &str) -> Result<Option<V>> {
        if !self.is_enabled() {
            return Ok(None);
        }

        let Some(path) = self.cache_path(key) else {
            return Ok(None);
        };

        if !tokio::fs::try_exists(&path)
            .await
            .with_context(|| format!("Failed to check cache file: {}", path.display()))?
        {
            return Ok(None);
        }

        let contents = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read cache file: {}", path.display()))?;

        let entry: CacheEntry<V> = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse cache file: {}", path.display()))?;

        Ok(Some(entry.data))
    }

    async fn set(&self, key: &str, value: &V) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let Some(path) = self.cache_path(key) else {
            return Ok(());
        };

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("Failed to create cache directory: {}", parent.display())
            })?;
        }

        let entry = CacheEntry::new(value);
        let contents =
            serde_json::to_string_pretty(&entry).context("Failed to serialize cache entry")?;

        // Atomic write: write to temp file, then rename
        let temp_path = path.with_extension("tmp");
        tokio::fs::write(&temp_path, contents)
            .await
            .with_context(|| format!("Failed to write cache temp file: {}", temp_path.display()))?;

        tokio::fs::rename(&temp_path, &path)
            .await
            .with_context(|| format!("Failed to rename cache file: {}", path.display()))?;

        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let Some(path) = self.cache_path(key) else {
            return Ok(());
        };

        if tokio::fs::try_exists(&path)
            .await
            .with_context(|| format!("Failed to check cache file: {}", path.display()))?
        {
            tokio::fs::remove_file(&path)
                .await
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
        assert!(dir.is_some());
        assert!(dir.unwrap().ends_with("aptu"));
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

    #[tokio::test]
    async fn test_file_cache_get_set() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let data = TestData {
            value: "test".to_string(),
            count: 42,
        };

        // Set value
        cache.set("test_key", &data).await.expect("set cache");

        // Get value
        let result = cache.get("test_key").await.expect("get cache");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);

        // Cleanup
        cache.remove("test_key").await.ok();
    }

    #[tokio::test]
    async fn test_file_cache_get_miss() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));

        let result = cache.get("nonexistent").await.expect("get cache");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_file_cache_get_stale() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::seconds(0));
        let data = TestData {
            value: "stale".to_string(),
            count: 99,
        };

        // Set value
        cache.set("stale_key", &data).await.expect("set cache");

        // Wait for TTL to expire
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // get() should return None (expired)
        let result = cache.get("stale_key").await.expect("get cache");
        assert!(result.is_none());

        // get_stale() should return the value
        let stale_result = cache.get_stale("stale_key").await.expect("get stale cache");
        assert!(stale_result.is_some());
        assert_eq!(stale_result.unwrap(), data);

        // Cleanup
        cache.remove("stale_key").await.ok();
    }

    #[tokio::test]
    async fn test_file_cache_remove() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let data = TestData {
            value: "remove_me".to_string(),
            count: 1,
        };

        // Set value
        cache.set("remove_key", &data).await.expect("set cache");

        // Verify it exists
        assert!(cache.get("remove_key").await.expect("get cache").is_some());

        // Remove it
        cache.remove("remove_key").await.expect("remove cache");

        // Verify it's gone
        assert!(cache.get("remove_key").await.expect("get cache").is_none());
    }

    #[tokio::test]
    #[should_panic(expected = "cache key must not contain path separators")]
    async fn test_cache_key_rejects_forward_slash() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let _ = cache.get("../etc/passwd").await;
    }

    #[tokio::test]
    #[should_panic(expected = "cache key must not contain path separators")]
    async fn test_cache_key_rejects_backslash() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let _ = cache.get("..\\windows\\system32").await;
    }

    #[tokio::test]
    #[should_panic(expected = "cache key must not contain path separators")]
    async fn test_cache_key_rejects_parent_dir() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_cache", Duration::hours(1));
        let _ = cache.get("foo..bar").await;
    }

    #[tokio::test]
    async fn test_disabled_cache_get_returns_none() {
        let cache: FileCacheImpl<TestData> =
            FileCacheImpl::with_dir(None, "test_cache", Duration::hours(1));
        let result = cache.get("any_key").await.expect("get should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_disabled_cache_set_succeeds_silently() {
        let cache: FileCacheImpl<TestData> =
            FileCacheImpl::with_dir(None, "test_cache", Duration::hours(1));
        let data = TestData {
            value: "test".to_string(),
            count: 42,
        };
        cache
            .set("any_key", &data)
            .await
            .expect("set should succeed");
    }

    #[tokio::test]
    async fn test_disabled_cache_remove_succeeds_silently() {
        let cache: FileCacheImpl<TestData> =
            FileCacheImpl::with_dir(None, "test_cache", Duration::hours(1));
        cache
            .remove("any_key")
            .await
            .expect("remove should succeed");
    }

    #[tokio::test]
    async fn test_disabled_cache_get_stale_returns_none() {
        let cache: FileCacheImpl<TestData> =
            FileCacheImpl::with_dir(None, "test_cache", Duration::hours(1));
        let result = cache
            .get_stale("any_key")
            .await
            .expect("get_stale should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_evict_stale_removes_old_files() {
        let cache: FileCacheImpl<TestData> = FileCacheImpl::new("test_evict", Duration::hours(1));
        let data = TestData {
            value: "old".to_string(),
            count: 1,
        };

        // Set a value
        cache.set("old_key", &data).await.expect("set cache");

        // Manually modify the cached_at timestamp to be old
        if let Some(path) = cache.cache_path("old_key") {
            let contents = tokio::fs::read_to_string(&path)
                .await
                .expect("read cache file");
            let mut entry: CacheEntry<TestData> =
                serde_json::from_str(&contents).expect("parse cache entry");
            entry.cached_at = Utc::now() - Duration::days(10);
            let new_contents = serde_json::to_string_pretty(&entry).expect("serialize cache entry");
            tokio::fs::write(&path, new_contents)
                .await
                .expect("write cache file");
        }

        // Evict files older than 7 days
        let evicted = cache.evict_stale(7).await;
        assert_eq!(evicted, 1);

        // Verify the file is gone
        let result = cache.get("old_key").await.expect("get cache");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_evict_stale_preserves_fresh_files() {
        let cache: FileCacheImpl<TestData> =
            FileCacheImpl::new("test_evict_fresh", Duration::hours(1));
        let data = TestData {
            value: "fresh".to_string(),
            count: 2,
        };

        // Set a value
        cache.set("fresh_key", &data).await.expect("set cache");

        // Evict files older than 7 days (this file is fresh, so it should be preserved)
        let evicted = cache.evict_stale(7).await;
        assert_eq!(evicted, 0);

        // Verify the file still exists
        let result = cache.get("fresh_key").await.expect("get cache");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);

        // Cleanup
        cache.remove("fresh_key").await.ok();
    }
}
