// SPDX-License-Identifier: Apache-2.0

//! Configuration management for the Aptu CLI.
//!
//! Provides layered configuration from files and environment variables.
//! Uses XDG-compliant paths via the `dirs` crate.
//!
//! # Configuration Sources (in priority order)
//!
//! 1. Environment variables (prefix: `APTU_`)
//! 2. Config file: `~/.config/aptu/config.toml`
//! 3. Built-in defaults
//!
//! # Examples
//!
//! ```bash
//! # Override AI model via environment variable
//! APTU_AI__MODEL=mistral-small cargo run
//! ```

pub mod ai;
pub mod cache;
pub mod loader;
pub mod review;

pub use ai::{AiConfig, FallbackConfig, FallbackEntry, TaskOverride, TaskType, TasksConfig};
pub use cache::{CacheConfig, ReposConfig};
#[cfg(not(target_arch = "wasm32"))]
pub use loader::TomlConfigSource;
pub use loader::{
    AppConfig, ConfigSource, GitHubConfig, InMemoryConfigSource, PromptConfig, UiConfig,
    UserConfig, config_dir, config_file_path, data_dir, load_config, prompts_dir,
};
pub use review::ReviewConfig;
