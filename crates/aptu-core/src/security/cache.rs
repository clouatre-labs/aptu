// SPDX-License-Identifier: Apache-2.0

//! Security finding cache for LLM validation results.
//!
//! Caches validated findings using SHA-256 hashes of (repo, file, pattern, snippet)
//! to avoid redundant LLM calls for identical findings across scans.

use anyhow::Result;
use chrono::Duration;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::instrument;

use crate::cache::{FileCache, FileCacheImpl};

use super::ValidatedFinding;

/// A cached security finding with validation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedFinding {
    /// The validated finding.
    pub validated: ValidatedFinding,
}

impl CachedFinding {
    /// Create a new cached finding.
    #[must_use]
    pub fn new(validated: ValidatedFinding) -> Self {
        Self { validated }
    }
}

/// Generate a cache key for a security finding.
///
/// Creates a SHA-256 hash of the concatenated components:
/// `{repo_owner}/{repo_name}:{file_path}:{pattern_id}:{matched_text}`
///
/// Uses incremental hashing to avoid allocating a large intermediate string,
/// which is more memory-efficient when `matched_text` contains large code snippets.
///
/// This ensures that identical findings across scans are cached,
/// while different contexts (repo, file, pattern, or code) produce unique keys.
///
/// # Arguments
///
/// * `repo_owner` - Repository owner (e.g., "octocat")
/// * `repo_name` - Repository name (e.g., "hello-world")
/// * `file_path` - File path where finding was detected
/// * `pattern_id` - Pattern ID that matched
/// * `matched_text` - The matched code snippet
///
/// # Returns
///
/// A 64-character hexadecimal SHA-256 hash.
#[must_use]
pub fn cache_key(
    repo_owner: &str,
    repo_name: &str,
    file_path: &str,
    pattern_id: &str,
    matched_text: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(repo_owner.as_bytes());
    hasher.update(b"/");
    hasher.update(repo_name.as_bytes());
    hasher.update(b":");
    hasher.update(file_path.as_bytes());
    hasher.update(b":");
    hasher.update(pattern_id.as_bytes());
    hasher.update(b":");
    hasher.update(matched_text.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Cache for security finding validation results.
///
/// Wraps `FileCacheImpl` with a 7-day TTL for validated findings.
/// Uses SHA-256 hashes as cache keys to ensure privacy and uniqueness.
pub struct FindingCache {
    cache: FileCacheImpl<CachedFinding>,
}

impl FindingCache {
    /// Create a new finding cache with default settings.
    ///
    /// Uses a 7-day TTL and stores cache files in `~/.cache/aptu/security`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: FileCacheImpl::new(
                "security",
                Duration::days(crate::cache::DEFAULT_SECURITY_TTL_DAYS),
            ),
        }
    }

    /// Get a cached validated finding.
    ///
    /// # Arguments
    ///
    /// * `repo_owner` - Repository owner
    /// * `repo_name` - Repository name
    /// * `file_path` - File path where finding was detected
    /// * `pattern_id` - Pattern ID that matched
    /// * `matched_text` - The matched code snippet
    ///
    /// # Returns
    ///
    /// The cached validated finding if it exists and is within TTL, `None` otherwise.
    #[instrument(skip(self, matched_text), fields(cache_key))]
    pub fn get(
        &self,
        repo_owner: &str,
        repo_name: &str,
        file_path: &str,
        pattern_id: &str,
        matched_text: &str,
    ) -> Result<Option<ValidatedFinding>> {
        let key = cache_key(repo_owner, repo_name, file_path, pattern_id, matched_text);
        tracing::Span::current().record("cache_key", &key);

        self.cache
            .get(&key)
            .map(|opt| opt.map(|cached| cached.validated))
    }

    /// Set a cached validated finding.
    ///
    /// # Arguments
    ///
    /// * `repo_owner` - Repository owner
    /// * `repo_name` - Repository name
    /// * `file_path` - File path where finding was detected
    /// * `pattern_id` - Pattern ID that matched
    /// * `matched_text` - The matched code snippet
    /// * `validated` - The validated finding to cache
    #[instrument(skip(self, matched_text, validated), fields(cache_key))]
    pub fn set(
        &self,
        repo_owner: &str,
        repo_name: &str,
        file_path: &str,
        pattern_id: &str,
        matched_text: &str,
        validated: ValidatedFinding,
    ) -> Result<()> {
        let key = cache_key(repo_owner, repo_name, file_path, pattern_id, matched_text);
        tracing::Span::current().record("cache_key", &key);

        let cached = CachedFinding::new(validated);
        self.cache.set(&key, &cached)
    }
}

impl Default for FindingCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{Confidence, Finding, Severity};

    #[test]
    fn test_cache_key_uniqueness() {
        // Different repos should produce different keys
        let key1 = cache_key("owner1", "repo1", "src/main.rs", "pattern1", "code");
        let key2 = cache_key("owner2", "repo1", "src/main.rs", "pattern1", "code");
        assert_ne!(key1, key2);

        // Different files should produce different keys
        let key3 = cache_key("owner1", "repo1", "src/lib.rs", "pattern1", "code");
        assert_ne!(key1, key3);

        // Different patterns should produce different keys
        let key4 = cache_key("owner1", "repo1", "src/main.rs", "pattern2", "code");
        assert_ne!(key1, key4);

        // Different code should produce different keys
        let key5 = cache_key("owner1", "repo1", "src/main.rs", "pattern1", "different");
        assert_ne!(key1, key5);

        // Identical inputs should produce identical keys
        let key6 = cache_key("owner1", "repo1", "src/main.rs", "pattern1", "code");
        assert_eq!(key1, key6);
    }

    #[test]
    fn test_cache_key_format() {
        let key = cache_key("owner", "repo", "file.rs", "pattern", "code");
        // SHA-256 produces 64 hex characters
        assert_eq!(key.len(), 64);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_cache_key_privacy() {
        // Cache key should not contain sensitive data
        let key = cache_key(
            "owner",
            "repo",
            "config.rs",
            "hardcoded-secret",
            "api_key = \"sk-secret123\"",
        );
        assert!(!key.contains("secret"));
        assert!(!key.contains("api_key"));
        assert!(!key.contains("sk-"));
    }

    #[test]
    fn test_finding_cache_hit() {
        let cache = FindingCache::new();
        let validated = ValidatedFinding {
            finding: Finding {
                pattern_id: "test-pattern".to_string(),
                description: "Test finding".to_string(),
                severity: Severity::High,
                confidence: Confidence::Medium,
                file_path: "src/test.rs".to_string(),
                line_number: 42,
                matched_text: "test code".to_string(),
                cwe: None,
            },
            is_valid: true,
            reasoning: "Test reasoning".to_string(),
            model_version: Some("test-model".to_string()),
        };

        // Set cache
        cache
            .set(
                "owner",
                "repo",
                "src/test.rs",
                "test-pattern",
                "test code",
                validated.clone(),
            )
            .expect("set cache");

        // Get cache hit
        let result = cache
            .get("owner", "repo", "src/test.rs", "test-pattern", "test code")
            .expect("get cache");

        assert!(result.is_some());
        assert_eq!(result.unwrap(), validated);

        // Cleanup
        let key = cache_key("owner", "repo", "src/test.rs", "test-pattern", "test code");
        cache.cache.remove(&key).ok();
    }

    #[test]
    fn test_finding_cache_miss() {
        let cache = FindingCache::new();

        let result = cache
            .get("owner", "repo", "src/nonexistent.rs", "pattern", "code")
            .expect("get cache");

        assert!(result.is_none());
    }

    #[test]
    fn test_finding_cache_different_context() {
        let cache = FindingCache::new();
        let validated = ValidatedFinding {
            finding: Finding {
                pattern_id: "pattern".to_string(),
                description: "Finding".to_string(),
                severity: Severity::Medium,
                confidence: Confidence::High,
                file_path: "src/file.rs".to_string(),
                line_number: 10,
                matched_text: "code".to_string(),
                cwe: None,
            },
            is_valid: false,
            reasoning: "False positive".to_string(),
            model_version: None,
        };

        // Set cache for one context
        cache
            .set(
                "owner1",
                "repo1",
                "src/file.rs",
                "pattern",
                "code",
                validated,
            )
            .expect("set cache");

        // Different owner should miss
        let result = cache
            .get("owner2", "repo1", "src/file.rs", "pattern", "code")
            .expect("get cache");
        assert!(result.is_none());

        // Cleanup
        let key = cache_key("owner1", "repo1", "src/file.rs", "pattern", "code");
        cache.cache.remove(&key).ok();
    }

    #[test]
    fn test_cached_finding_serialization() {
        let validated = ValidatedFinding {
            finding: Finding {
                pattern_id: "test".to_string(),
                description: "Test".to_string(),
                severity: Severity::Low,
                confidence: Confidence::Low,
                file_path: "test.rs".to_string(),
                line_number: 1,
                matched_text: "test".to_string(),
                cwe: Some("CWE-123".to_string()),
            },
            is_valid: true,
            reasoning: "Valid".to_string(),
            model_version: Some("model-v1".to_string()),
        };

        let cached = CachedFinding::new(validated.clone());
        let json = serde_json::to_string(&cached).expect("serialize");
        let deserialized: CachedFinding = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(cached, deserialized);
        assert_eq!(deserialized.validated, validated);
    }
}
