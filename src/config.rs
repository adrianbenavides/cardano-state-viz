//! Configuration management
//!
//! This module handles loading and managing configuration from:
//! - Command-line arguments
//! - Environment variables
//! - Configuration files (TOML)
//! - Defaults

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Top-level configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub default: DefaultConfig,

    #[serde(default)]
    pub blockfrost: BlockfrostConfig,

    #[serde(default)]
    pub node: NodeConfig,

    #[serde(default)]
    pub cache: CacheConfig,

    #[serde(default)]
    pub logging: LoggingConfig,
}

/// Default settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultConfig {
    #[serde(default = "default_source")]
    pub source: String,

    #[serde(default = "default_network")]
    pub network: String,
}

/// Blockfrost API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockfrostConfig {
    /// Blockfrost API key
    pub api_key: Option<String>,

    /// Enable caching
    #[serde(default)]
    pub cache_enabled: bool,

    /// Cache TTL
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: Duration,

    /// Maximum number of retries for API requests
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Delay between retries
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: Duration,
}

/// Cardano node configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeConfig {
    /// Path to node socket
    pub socket_path: Option<PathBuf>,

    /// Network magic number
    pub network_magic: Option<u32>,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    #[serde(default)]
    pub enabled: bool,

    /// Cache directory
    pub directory: Option<PathBuf>,

    /// Cache TTL
    #[serde(default = "default_cache_ttl")]
    pub ttl: Duration,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log file path
    pub file: Option<PathBuf>,
}

// Default value functions

fn default_source() -> String {
    "mock".to_string()
}

fn default_network() -> String {
    "preprod".to_string()
}

fn default_cache_ttl() -> Duration {
    Duration::from_hours(1)
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay() -> Duration {
    Duration::from_millis(1000)
}

fn default_log_level() -> String {
    "info".to_string()
}

// Default implementations

impl Default for DefaultConfig {
    fn default() -> Self {
        Self {
            source: default_source(),
            network: default_network(),
        }
    }
}

impl Default for BlockfrostConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            cache_enabled: false,
            cache_ttl: default_cache_ttl(),
            max_retries: default_max_retries(),
            retry_delay_ms: default_retry_delay(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            directory: None,
            ttl: default_cache_ttl(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn from_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| Error::Config(format!("Failed to read config file {:?}: {}", path, e)))?;

        let config: Config = toml::from_str(&contents)
            .map_err(|e| Error::Config(format!("Failed to parse config file {:?}: {}", path, e)))?;

        Ok(config)
    }

    /// Load configuration from default locations
    ///
    /// Searches in order:
    /// 1. ./config.toml
    /// 2. ~/.cardano-state-viz/config.toml
    /// 3. /etc/cardano-state-viz/config.toml
    pub fn load() -> Result<Self> {
        let paths = vec![
            PathBuf::from("config.toml"),
            dirs::home_dir()
                .map(|h| h.join(".cardano-state-viz").join("config.toml"))
                .unwrap_or_else(|| PathBuf::from("/dev/null")),
            PathBuf::from("/etc/cardano-state-viz/config.toml"),
        ];

        for path in paths {
            if path.exists() {
                tracing::info!("Loading config from {:?}", path);
                return Self::from_file(path);
            }
        }

        tracing::info!("No config file found, using defaults");
        Ok(Config::default())
    }

    /// Get Blockfrost API key from config or environment
    pub fn blockfrost_api_key(&self) -> Result<String> {
        // TODO: revert -> Priority: config file > environment variable
        if let Some(key) = &self.blockfrost.api_key {
            return Ok(key.clone());
        }

        std::env::var("BLOCKFROST_API_KEY")
            .map_err(|_| Error::MissingConfig(
                "Blockfrost API key not found. Set BLOCKFROST_API_KEY environment variable or configure in config file".to_string()
            ))
    }

    /// Get node socket path from config or environment
    pub fn node_socket_path(&self) -> Result<PathBuf> {
        // TODO: revert -> config file > environment variable
        if let Some(path) = &self.node.socket_path {
            return Ok(path.clone());
        }

        std::env::var("CARDANO_NODE_SOCKET_PATH")
            .map(PathBuf::from)
            .map_err(|_| Error::MissingConfig(
                "Node socket path not found. Set CARDANO_NODE_SOCKET_PATH environment variable or configure in config file".to_string()
            ))
    }

    /// Get cache directory with fallback to default
    pub fn cache_directory(&self) -> PathBuf {
        self.cache.directory.clone().unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".cardano-state-viz").join("cache"))
                .unwrap_or_else(|| PathBuf::from("/tmp/cardano-state-viz-cache"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default.source, "mock");
        assert_eq!(config.default.network, "preprod");
        assert_eq!(config.logging.level, "info");
    }

    #[test]
    fn test_parse_toml_config() {
        let toml = r#"
[default]
source = "blockfrost"
network = "mainnet"

[blockfrost]
api_key = "test_key"
network = "mainnet"

[logging]
level = "debug"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.default.source, "blockfrost");
        assert_eq!(config.blockfrost.api_key, Some("test_key".to_string()));
        assert_eq!(config.logging.level, "debug");
    }
}
