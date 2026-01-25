// SPDX-License-Identifier: Apache-2.0

//! Security scanning module for vulnerability detection.
//!
//! Provides pattern-based security scanning for pull requests and code changes.
//! Uses regex patterns to detect common vulnerabilities like hardcoded secrets,
//! SQL injection, XSS, and other OWASP/CWE issues.

pub mod detection;
pub mod patterns;
pub mod scanner;
pub mod types;

pub use detection::needs_security_scan;
pub use patterns::PatternEngine;
pub use scanner::SecurityScanner;
pub use types::{Confidence, Finding, PatternDefinition, Severity};
