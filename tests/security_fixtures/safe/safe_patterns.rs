// SPDX-License-Identifier: Apache-2.0

//! Test fixture with safe code patterns (no vulnerabilities).
//!
//! This file should NOT trigger any security findings.

#![allow(dead_code)]

use std::env;

/// Safe configuration loading from environment.
fn load_config() -> Config {
    Config {
        api_key: env::var("API_KEY").expect("API_KEY not set"),
        database_url: env::var("DATABASE_URL").expect("DATABASE_URL not set"),
    }
}

struct Config {
    api_key: String,
    database_url: String,
}

/// Safe database query with parameterized statements.
fn query_user(id: &str) -> Result<User, Error> {
    let query = "SELECT * FROM users WHERE id = ?";
    execute_with_params(query, &[id])
}

/// Safe password hashing with modern algorithm.
fn hash_password(password: &str) -> String {
    // Using SHA-256 or better (not MD5/SHA1)
    sha256(password)
}

/// Safe random number generation.
fn generate_token() -> String {
    use std::sync::OsRng;
    // Using cryptographically secure RNG
    OsRng.gen::<u64>().to_string()
}

/// Safe file operations without path traversal.
fn read_user_file(filename: &str) -> Result<String, Error> {
    // Validate filename doesn't contain path traversal
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(Error::InvalidPath);
    }
    
    let safe_path = format!("/var/app/uploads/{}", filename);
    std::fs::read_to_string(safe_path).map_err(|_| Error::FileNotFound)
}

/// Safe HTML rendering with proper escaping.
fn render_user_content(content: &str) -> String {
    // Using proper HTML escaping, not innerHTML
    html_escape(content)
}

// Mock types and functions
struct User;
enum Error {
    InvalidPath,
    FileNotFound,
}

fn execute_with_params(_query: &str, _params: &[&str]) -> Result<User, Error> {
    Ok(User)
}

fn sha256(_data: &str) -> String {
    String::new()
}

fn html_escape(content: &str) -> String {
    content.replace('<', "&lt;").replace('>', "&gt;")
}

trait OsRng {
    fn gen<T>(&self) -> T;
}

impl OsRng for std::sync::OsRng {
    fn gen<T>(&self) -> T {
        unimplemented!()
    }
}
