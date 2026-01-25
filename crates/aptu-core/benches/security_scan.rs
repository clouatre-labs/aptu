// SPDX-License-Identifier: Apache-2.0

//! Benchmark for security scanning performance.
//!
//! Validates that pattern matching completes in <10ms for typical code samples.

use aptu_core::security::SecurityScanner;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

/// Generate a realistic code sample with ~500 lines.
fn generate_test_code() -> String {
    let mut code = String::new();

    // Add some safe code
    for i in 0..100 {
        code.push_str("fn function_");
        code.push_str(&i.to_string());
        code.push_str("() {\n");
        code.push_str("    let config = load_config();\n");
        code.push_str("    let result = process_data(&config);\n");
        code.push_str("    Ok(result)\n");
        code.push_str("}\n\n");
    }

    code
}

/// Generate a code sample with some vulnerabilities.
fn generate_vulnerable_code() -> String {
    let mut code = String::new();

    // Mix of safe and vulnerable code
    for i in 0..50 {
        code.push_str("fn function_");
        code.push_str(&i.to_string());
        code.push_str("() {\n");
        code.push_str("    let config = load_config();\n");
        code.push_str("    let result = process_data(&config);\n");
        code.push_str("    Ok(result)\n");
        code.push_str("}\n\n");
    }

    // Add some vulnerabilities
    code.push_str("fn vulnerable_function() {\n");
    code.push_str("    let api_key = \"sk-1234567890abcdefghijklmnopqrstuvwxyz\";\n");
    code.push_str("    let password = \"hardcoded123\";\n");
    code.push_str("    query(\"SELECT * FROM users WHERE id = \" + user_input);\n");
    code.push_str("    let hash = md5(data);\n");
    code.push_str("}\n\n");

    // More safe code
    for i in 50..100 {
        code.push_str("fn function_");
        code.push_str(&i.to_string());
        code.push_str("() {\n");
        code.push_str("    let config = load_config();\n");
        code.push_str("    let result = process_data(&config);\n");
        code.push_str("    Ok(result)\n");
        code.push_str("}\n\n");
    }

    code
}

fn bench_scan_safe_code(c: &mut Criterion) {
    let scanner = SecurityScanner::new();
    let code = generate_test_code();

    c.bench_function("scan_safe_code_500_lines", |b| {
        b.iter(|| scanner.scan_file(black_box(&code), black_box("test.rs")));
    });
}

fn bench_scan_vulnerable_code(c: &mut Criterion) {
    let scanner = SecurityScanner::new();
    let code = generate_vulnerable_code();

    c.bench_function("scan_vulnerable_code_500_lines", |b| {
        b.iter(|| scanner.scan_file(black_box(&code), black_box("test.rs")));
    });
}

fn bench_scan_diff(c: &mut Criterion) {
    let scanner = SecurityScanner::new();
    let diff = r#"
diff --git a/src/config.rs b/src/config.rs
--- a/src/config.rs
+++ b/src/config.rs
@@ -1,10 +1,15 @@
 fn load_config() {
     let host = "localhost";
     let port = 8080;
+    let api_key = "sk-1234567890abcdefghijklmnopqrstuvwxyz";
+    let password = "hardcoded123";
 }
 
 fn query_user(id: &str) {
-    let sql = format!("SELECT * FROM users WHERE id = ?");
+    let sql = "SELECT * FROM users WHERE id = " + id;
     execute(&sql);
 }
"#;

    c.bench_function("scan_diff_small", |b| {
        b.iter(|| scanner.scan_diff(black_box(diff)));
    });
}

criterion_group!(
    benches,
    bench_scan_safe_code,
    bench_scan_vulnerable_code,
    bench_scan_diff
);
criterion_main!(benches);
