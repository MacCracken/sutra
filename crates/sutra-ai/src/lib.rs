//! sutra-ai — AI integration: Markdown/NL to TOML translation, daimon/hoosh clients.

pub mod daimon;
pub mod markdown;

// Re-export converter from core
pub use sutra_core::{toml_to_yaml, yaml_to_toml};
