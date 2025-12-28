// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2025 Aptu Contributors

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = toml::from_str::<aptu_core::repos::custom::CustomReposFile>(s);
    }
});
