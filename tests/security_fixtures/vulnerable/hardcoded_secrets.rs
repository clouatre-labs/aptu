// SPDX-License-Identifier: Apache-2.0

//! Test fixture with intentional hardcoded secrets vulnerabilities.
//!
//! WARNING: This file contains intentionally vulnerable code for testing purposes.
//! DO NOT use these patterns in production code.

#![allow(dead_code)]

/// Example with hardcoded API key (CWE-798).
fn hardcoded_api_key() {
    let api_key = "sk-1234567890abcdefghijklmnopqrstuvwxyz";
    let secret_key = "secret_abcdefghijklmnopqrstuvwxyz1234567890";
    let access_token = "ghp_1234567890abcdefghijklmnopqrstuvwxyz";
    
    println!("Using API key: {}", api_key);
    println!("Using secret: {}", secret_key);
    println!("Using token: {}", access_token);
}

/// Example with hardcoded password (CWE-798).
fn hardcoded_password() {
    let password = "SuperSecret123!";
    let passwd = "admin12345678";
    let pwd = "MyPassword2024";
    
    authenticate(password);
    login(passwd);
    verify(pwd);
}

fn authenticate(_password: &str) {}
fn login(_passwd: &str) {}
fn verify(_pwd: &str) {}
