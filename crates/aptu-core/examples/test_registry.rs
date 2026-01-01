// SPDX-License-Identifier: Apache-2.0

//! Quick test to verify the static registry works

use aptu_core::ai::registry::{all_providers, get_provider};

fn main() {
    println!("=== Testing Static Registry ===\n");

    // Test 1: List all providers
    println!("Test 1: All Providers");
    let providers = all_providers();
    println!("Found {} providers:", providers.len());
    for p in providers {
        println!(
            "  - {} ({}) - {} models",
            p.display_name,
            p.name,
            p.models.len()
        );
    }

    // Test 2: Get specific provider
    println!("\nTest 2: Get OpenRouter Provider");
    if let Some(provider) = get_provider("openrouter") {
        println!("Provider: {}", provider.display_name);
        println!("API URL: {}", provider.api_url);
        println!("Models:");
        for model in provider.models {
            println!("  - {} ({})", model.display_name, model.identifier);
            println!(
                "    Free: {}, Context: {}",
                model.is_free, model.context_window
            );
        }
    }

    // Test 3: Get Gemini provider
    println!("\nTest 3: Get Gemini Provider");
    if let Some(provider) = get_provider("gemini") {
        println!("Provider: {}", provider.display_name);
        println!("Models:");
        for model in provider.models {
            println!("  - {} ({})", model.display_name, model.identifier);
        }
    }
}
