//! Caching component
//!
//! This module provides a caching mechanism for storing and retrieving
//! transaction data and raw text responses.

use crate::{data_source::Transaction, Result};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tokio::fs;

/// Data Source Cache
#[derive(Debug, Clone)]
pub struct DataSourceCache {
    cache_dir: PathBuf,
    ttl: Duration,
}

impl DataSourceCache {
    /// Create a new cache instance
    pub fn new(ttl: Duration, cache_dir: Option<PathBuf>) -> Self {
        let cache_dir = cache_dir.unwrap_or_else(|| {
            dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("cardano-state-viz")
        });

        Self { cache_dir, ttl }
    }

    pub fn cache_key_for(address: &str, page: usize, page_size: usize, order: &str) -> String {
        format!("addr_txs_{}_{}_{}_{}", address, page, page_size, order)
    }

    /// Get cache file path
    fn get_cache_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.json", key))
    }

    /// Check if cache is valid
    async fn is_cache_valid(&self, path: &PathBuf) -> bool {
        if !path.exists() {
            return false;
        }

        match fs::metadata(path).await {
            Ok(metadata) => match metadata.modified() {
                Ok(modified) => {
                    if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                        elapsed < self.ttl
                    } else {
                        false
                    }
                }
                Err(_) => false,
            },
            Err(_) => false,
        }
    }

    /// Ensure cache directory exists
    async fn ensure_cache_dir(&self) -> Result<()> {
        if !self.cache_dir.exists() {
            fs::create_dir_all(&self.cache_dir)
                .await
                .map_err(crate::Error::Io)?;
        }
        Ok(())
    }

    /// Get a full transaction from cache
    pub async fn get_transaction(&self, tx_hash: &str) -> Option<Transaction> {
        let key = format!("tx_{}", tx_hash);
        let path = self.get_cache_path(&key);

        if self.is_cache_valid(&path).await {
            match fs::read_to_string(&path).await {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(tx) => {
                        tracing::debug!("Cache hit for transaction {}", tx_hash);
                        return Some(tx);
                    }
                    Err(e) => tracing::warn!("Failed to deserialize cached transaction: {}", e),
                },
                Err(e) => tracing::warn!("Failed to read cached transaction: {}", e),
            }
        }
        None
    }

    /// Save a full transaction to cache
    pub async fn save_transaction(&self, tx: &Transaction) {
        if let Err(e) = self.ensure_cache_dir().await {
            tracing::warn!("Failed to create cache directory: {}", e);
            return;
        }

        let key = format!("tx_{}", tx.hash);
        let path = self.get_cache_path(&key);

        match serde_json::to_string(tx) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json).await {
                    tracing::warn!("Failed to write transaction to cache: {}", e);
                }
            }
            Err(e) => tracing::warn!("Failed to serialize transaction: {}", e),
        }
    }

    /// Get raw text content from cache (useful for pages)
    pub async fn get_text(&self, key: &str) -> Option<String> {
        let path = self.get_cache_path(key);
        if self.is_cache_valid(&path).await {
            match fs::read_to_string(&path).await {
                Ok(content) => {
                    tracing::debug!("Cache hit for key {}", key);
                    return Some(content);
                }
                Err(e) => tracing::warn!("Failed to read cached text: {}", e),
            }
        }
        None
    }

    /// Save raw text content to cache
    pub async fn save_text(&self, key: &str, content: &str) {
        if let Err(e) = self.ensure_cache_dir().await {
            tracing::warn!("Failed to create cache directory: {}", e);
            return;
        }

        let path = self.get_cache_path(key);
        if let Err(e) = fs::write(&path, content).await {
            tracing::warn!("Failed to write text to cache: {}", e);
        }
    }
}
