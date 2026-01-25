// SPDX-License-Identifier: Apache-2.0

//! Security scanning module for vulnerability detection.
//!
//! Provides pattern-based security scanning for pull requests and code changes.
//! Uses regex patterns to detect common vulnerabilities like hardcoded secrets,
//! SQL injection, XSS, and other OWASP/CWE issues.

pub mod cache;
pub mod detection;
pub mod ignore;
pub mod patterns;
pub mod sarif;
pub mod scanner;
pub mod types;
pub mod validator;

pub use cache::{CachedFinding, FindingCache, cache_key};
pub use detection::needs_security_scan;
pub use ignore::SecurityConfig;
pub use patterns::PatternEngine;
pub use sarif::SarifReport;
pub use scanner::SecurityScanner;
pub use types::{
    Confidence, Finding, PatternDefinition, Severity, ValidatedFinding, ValidationResult,
};
pub use validator::SecurityValidator;
