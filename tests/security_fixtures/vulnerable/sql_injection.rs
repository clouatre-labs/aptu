// SPDX-License-Identifier: Apache-2.0

//! Test fixture with intentional SQL injection vulnerabilities.
//!
//! WARNING: This file contains intentionally vulnerable code for testing purposes.
//! DO NOT use these patterns in production code.

#![allow(dead_code)]

/// SQL injection via string concatenation (CWE-89).
fn sql_injection_concat(user_id: &str) {
    let query = "SELECT * FROM users WHERE id = " + user_id;
    execute(query);
    
    let delete_query = "DELETE FROM sessions WHERE user_id = " + user_id;
    execute(delete_query);
}

/// SQL injection via format string (CWE-89).
fn sql_injection_format(username: &str, table: &str) {
    let query = format!("SELECT * FROM {} WHERE username = '{}'", table, username);
    execute(query);
    
    let update = format!("UPDATE users SET active = 1 WHERE name = '{}'", username);
    execute(update);
}

/// Command injection (CWE-78).
fn command_injection(filename: &str) {
    let cmd = "cat /var/log/" + filename;
    system(cmd);
    
    let exec_cmd = "rm -rf " + filename;
    exec(exec_cmd);
}

/// Weak cryptography (CWE-327).
fn weak_crypto(data: &str) {
    let hash1 = md5(data);
    let hash2 = SHA1(data);
    
    println!("MD5: {}", hash1);
    println!("SHA1: {}", hash2);
}

fn execute(_query: &str) {}
fn system(_cmd: &str) {}
fn exec(_cmd: &str) {}
fn md5(_data: &str) -> String { String::new() }
fn SHA1(_data: &str) -> String { String::new() }
