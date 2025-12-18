//! Data source module - Abstraction for fetching Cardano transaction data
//!
//! This module provides a trait-based abstraction for fetching transaction data
//! from multiple sources (Blockfrost API, Cardano node, mock data).

use crate::{Config, Result};
use async_trait::async_trait;

pub mod blockfrost;
pub mod mock;
pub mod models;
pub mod node;

// Re-export models
use crate::cli::DataSourceType;
pub use models::{
    Asset, Datum, ExUnits, Metadata, QueryParams, Redeemer, RedeemerTag, Script, Transaction,
    TxInput, TxOutput, UtxoRef, Witnesses,
};

/// Data source trait for fetching Cardano blockchain data
///
/// Implementations provide different backends for accessing transaction data:
/// - `BlockfrostDataSource`: Uses Blockfrost API
/// - `NodeDataSource`: Connects to local Cardano node
/// - `MockDataSource`: Provides hardcoded test data
#[async_trait]
pub trait DataSource: Send + Sync {
    /// Fetch a single transaction by hash
    async fn get_transaction(&self, tx_hash: &str) -> Result<Transaction>;

    /// Fetch all transactions for a given address
    async fn get_transactions_by_address(
        &self,
        address: &str, // TODO: should use pallas::addresses::Address?
        params: QueryParams,
    ) -> Result<Vec<Transaction>>;

    /// Fetch current UTXOs at a script address
    async fn get_utxos_at_address(&self, address: &str) -> Result<Vec<(UtxoRef, TxOutput)>>;
}

/// Create a data source instance based on type and configuration
pub async fn create_data_source(
    source_type: DataSourceType,
    config: &Config,
) -> Result<Box<dyn DataSource>> {
    match source_type {
        DataSourceType::Mock => Ok(Box::new(mock::MockDataSource::new())),
        DataSourceType::Blockfrost => {
            let api_key = config.blockfrost_api_key()?;
            let network = config.blockfrost.network.clone();
            Ok(Box::new(
                blockfrost::BlockfrostDataSource::new(api_key, network)?
                    .with_max_retries(config.blockfrost.max_retries)
                    .with_retry_delay(config.blockfrost.retry_delay_ms),
            ))
        }
        DataSourceType::Node => {
            let socket_path = config.node_socket_path()?;
            let network_magic = config.node.network_magic;
            Ok(Box::new(node::NodeDataSource::new(
                socket_path,
                network_magic,
            )))
        }
    }
}
