//! Centralized configuration for the CVM Agent.
//!
//! Configuration is loaded from environment variables with sensible defaults.
//! All paths and settings that were previously hardcoded are now configurable.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Global configuration instance.
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Main configuration structure for the CVM Agent.
#[derive(Debug, Clone)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,
    /// Database configuration
    pub db: DbConfig,
    /// Skills configuration
    pub skills: SkillsConfig,
    /// Working directory configuration
    pub paths: PathsConfig,
    /// LLM configuration (used in Phase 4 for service injection)
    #[allow(dead_code)]
    pub llm: LlmConfig,
}

/// Server-related configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address to listen on (default: "0.0.0.0:8080")
    pub listen_addr: String,
}

/// Database configuration.
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Path to the SQLite database file
    pub path: PathBuf,
}

/// Skills module configuration.
#[derive(Debug, Clone)]
pub struct SkillsConfig {
    /// Directory containing skill definitions
    pub dir: PathBuf,
}

/// Path configuration for various operations.
#[derive(Debug, Clone)]
pub struct PathsConfig {
    /// Working directory for git and shell operations
    pub working_dir: PathBuf,
    /// Flake reference for NixOS operations (e.g., "/app#phala-cvm")
    pub flake_ref: String,
    /// Configuration directory for user preferences and setup state
    pub config_dir: PathBuf,
}

/// LLM (Language Model) configuration.
/// Note: Will be used in Phase 4 for service injection.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// API key for the LLM service
    pub api_key: Option<String>,
    /// Base URL for the LLM API
    pub base_url: String,
    /// Model identifier to use
    pub model: String,
}

impl Config {
    /// Loads configuration from environment variables.
    ///
    /// Environment variables:
    /// - `AGENT_LISTEN_ADDR`: Server listen address (default: "0.0.0.0:8080")
    /// - `AGENT_DB_PATH`: Database file path (default: "./data/agent.db")
    /// - `AGENT_SKILLS_DIR`: Skills directory (default: "/app/cvm-agent/skills")
    /// - `AGENT_WORKING_DIR`: Working directory for operations (default: "/app")
    /// - `AGENT_FLAKE_REF`: NixOS flake reference (default: "/app#phala-cvm")
    /// - `REDPILL_API_KEY` or `ANTHROPIC_AUTH_TOKEN`: LLM API key
    /// - `REDPILL_BASE_URL` or `ANTHROPIC_BASE_URL`: LLM base URL
    /// - `REDPILL_MODEL` or `ANTHROPIC_MODEL`: LLM model name
    pub fn from_env() -> Self {
        Self {
            server: ServerConfig {
                listen_addr: std::env::var("AGENT_LISTEN_ADDR")
                    .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            },
            db: DbConfig {
                path: PathBuf::from(
                    std::env::var("AGENT_DB_PATH")
                        .unwrap_or_else(|_| "./data/agent.db".to_string()),
                ),
            },
            skills: SkillsConfig {
                dir: PathBuf::from(
                    std::env::var("AGENT_SKILLS_DIR")
                        .unwrap_or_else(|_| "/app/cvm-agent/skills".to_string()),
                ),
            },
            paths: PathsConfig {
                working_dir: PathBuf::from(
                    std::env::var("AGENT_WORKING_DIR")
                        .unwrap_or_else(|_| "/app".to_string()),
                ),
                flake_ref: std::env::var("AGENT_FLAKE_REF")
                    .unwrap_or_else(|_| "/app#phala-cvm".to_string()),
                config_dir: PathBuf::from(
                    std::env::var("AGENT_CONFIG_DIR")
                        .unwrap_or_else(|_| {
                            dirs::home_dir()
                                .map(|h| h.join(".cvm-agent"))
                                .unwrap_or_else(|| PathBuf::from("/tmp/.cvm-agent"))
                                .to_string_lossy()
                                .to_string()
                        }),
                ),
            },
            llm: LlmConfig {
                api_key: std::env::var("REDPILL_API_KEY")
                    .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
                    .ok(),
                base_url: std::env::var("REDPILL_BASE_URL")
                    .or_else(|_| std::env::var("ANTHROPIC_BASE_URL"))
                    .unwrap_or_else(|_| "https://api.redpill.ai".to_string()),
                model: std::env::var("REDPILL_MODEL")
                    .or_else(|_| std::env::var("ANTHROPIC_MODEL"))
                    .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string()),
            },
        }
    }

    /// Initializes the global configuration.
    ///
    /// This should be called once at startup. Subsequent calls will return
    /// the already-initialized configuration.
    pub fn init() -> &'static Config {
        CONFIG.get_or_init(|| Self::from_env())
    }

    /// Gets the global configuration.
    ///
    /// Panics if `init()` has not been called.
    pub fn get() -> &'static Config {
        CONFIG.get().expect("Config not initialized. Call Config::init() first.")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        // Clear any env vars that might interfere
        std::env::remove_var("AGENT_LISTEN_ADDR");
        std::env::remove_var("AGENT_DB_PATH");
        std::env::remove_var("AGENT_SKILLS_DIR");
        std::env::remove_var("AGENT_WORKING_DIR");
        std::env::remove_var("AGENT_FLAKE_REF");

        let config = Config::from_env();

        assert_eq!(config.server.listen_addr, "0.0.0.0:8080");
        assert_eq!(config.db.path, PathBuf::from("./data/agent.db"));
        assert_eq!(config.skills.dir, PathBuf::from("/app/cvm-agent/skills"));
        assert_eq!(config.paths.working_dir, PathBuf::from("/app"));
        assert_eq!(config.paths.flake_ref, "/app#phala-cvm");
        assert_eq!(config.llm.base_url, "https://api.redpill.ai");
        assert_eq!(config.llm.model, "claude-3-5-sonnet-20241022");
    }

    #[test]
    fn test_config_from_env() {
        std::env::set_var("AGENT_LISTEN_ADDR", "127.0.0.1:9000");
        std::env::set_var("AGENT_WORKING_DIR", "/custom/path");

        let config = Config::from_env();

        assert_eq!(config.server.listen_addr, "127.0.0.1:9000");
        assert_eq!(config.paths.working_dir, PathBuf::from("/custom/path"));

        // Clean up
        std::env::remove_var("AGENT_LISTEN_ADDR");
        std::env::remove_var("AGENT_WORKING_DIR");
    }
}
