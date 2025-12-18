//! Blockfrost API data source implementation
//!
//! This module provides integration with the Blockfrost API for fetching
//! real Cardano blockchain data.
//!
//! Note: This is a simplified implementation. Full Blockfrost integration requires
//! detailed mapping from blockfrost-openapi types to our internal types.

use super::{
    Asset, DataSource, Datum, ExUnits, QueryParams, Redeemer, RedeemerTag, Script, Transaction,
    TxInput, TxOutput, UtxoRef, Witnesses,
};
use crate::{Error, Result};
use async_trait::async_trait;
use blockfrost::{BlockFrostSettings, BlockfrostAPI, Order, Pagination};
use std::time::Duration;
use tokio::time::sleep;

/// Calculate Blake2b-256 hash of data
fn blake2b_256(data: &[u8]) -> String {
    use blake2::digest::consts::U32;
    use blake2::{Blake2b, Digest};

    let mut hasher = Blake2b::<U32>::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Blockfrost API client with rate limiting and retry logic
#[derive(Debug)]
pub struct BlockfrostDataSource {
    client: BlockfrostAPI,
    network: String,
    max_retries: u32,
    retry_delay: Duration,
}

impl BlockfrostDataSource {
    /// Create a new Blockfrost data source
    pub fn new(
        api_key: String,
        network: String, // TODO: enum with free variant
    ) -> Result<Self> {
        // Validate network
        if !matches!(
            network.as_str(),
            "mainnet" | "preprod" | "preview" | "testnet"
        ) {
            return Err(Error::Config(format!(
                "Invalid network '{}'. Must be one of: mainnet, preprod, preview, testnet",
                network
            )));
        }

        // Create Blockfrost client with settings
        let settings = BlockFrostSettings::new();
        let client = BlockfrostAPI::new(&api_key, settings);

        Ok(Self {
            client,
            network,
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
        })
    }

    /// Set maximum retry attempts
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set retry delay in milliseconds
    pub fn with_retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    /// Execute a Blockfrost API call with retry logic
    async fn execute_with_retry<F, T, Fut>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = blockfrost::error::BlockfrostResult<T>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(
                    // Exponential backoff
                    (self.retry_delay.as_millis() * (1 << (attempt - 1))) as u64,
                );
                tracing::debug!("Retrying after {:?} (attempt {})", delay, attempt);
                sleep(delay).await;
            }

            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    tracing::warn!(
                        "Blockfrost API error (attempt {}): {:?}",
                        attempt + 1,
                        last_error
                    );

                    // Don't retry on certain errors
                    if let Some(ref err) = last_error {
                        let err_str = format!("{:?}", err);
                        if err_str.contains("404") || err_str.contains("Invalid") {
                            break; // Don't retry on not found or invalid requests
                        }
                    }
                }
            }
        }

        Err(Error::Blockfrost(format!(
            "Failed after {} attempts: {:?}",
            self.max_retries + 1,
            last_error
        )))
    }

    /// Fetch a single transaction with all details (static helper for concurrency)
    async fn fetch_full_transaction(
        client: BlockfrostAPI,
        tx_hash: String,
        max_retries: u32,
        retry_delay: Duration,
    ) -> Result<Transaction> {
        tracing::debug!("Fetching transaction {} from Blockfrost", tx_hash);

        // Fetch details, UTXOs, and redeemers concurrently
        let c1 = client.clone();
        let h1 = tx_hash.clone();
        let details_future = Self::fetch_with_retry(
            move || {
                let c = c1.clone();
                let h = h1.clone();
                async move { c.transaction_by_hash(&h).await }
            },
            max_retries,
            retry_delay,
            "transaction_by_hash",
        );

        let c2 = client.clone();
        let h2 = tx_hash.clone();
        let utxos_future = Self::fetch_with_retry(
            move || {
                let c = c2.clone();
                let h = h2.clone();
                async move { c.transactions_utxos(&h).await }
            },
            max_retries,
            retry_delay,
            "transactions_utxos",
        );

        let c3 = client.clone();
        let h3 = tx_hash.clone();
        let redeemers_future = Self::fetch_with_retry(
            move || {
                let c = c3.clone();
                let h = h3.clone();
                async move { c.transactions_redeemers(&h).await }
            },
            max_retries,
            retry_delay,
            "transactions_redeemers",
        );

        let (tx_result, utxos_result, redeemers_result) =
            tokio::join!(details_future, utxos_future, redeemers_future);

        let tx = tx_result?;
        let utxos = utxos_result?;
        let redeemers = redeemers_result.unwrap_or_else(|e| {
            tracing::warn!("Failed to fetch redeemers for {}: {}", tx_hash, e);
            vec![]
        });

        // Map inputs
        let inputs = utxos
            .inputs
            .iter()
            .map(|input| {
                let amount: Vec<Asset> = input
                    .amount
                    .iter()
                    .map(|a| Asset {
                        unit: a.unit.clone(),
                        quantity: a.quantity.clone(),
                    })
                    .collect();

                TxInput {
                    utxo_ref: UtxoRef::new(input.tx_hash.clone(), input.output_index as u64),
                    address: Some(input.address.clone()),
                    amount: Some(amount),
                }
            })
            .collect();

        // Map outputs
        let outputs = utxos
            .outputs
            .iter()
            .map(|output| {
                let amount: Vec<Asset> = output
                    .amount
                    .iter()
                    .map(|a| Asset {
                        unit: a.unit.clone(),
                        quantity: a.quantity.clone(),
                    })
                    .collect();

                let datum = if let Some(ref inline_datum) = output.inline_datum {
                    Some(Self::parse_datum_from_hex_static(inline_datum))
                } else {
                    output.data_hash.as_ref().map(|data_hash| Datum {
                        hash: data_hash.clone(),
                        raw_cbor: vec![],
                        parsed: None,
                    })
                };

                TxOutput {
                    address: output.address.clone(),
                    amount,
                    datum,
                    script_ref: output.reference_script_hash.as_ref().map(|_hash| Script {
                        script_type: "Unknown".to_string(),
                        bytes: vec![],
                    }),
                }
            })
            .collect();

        let mapped_redeemers: Vec<Redeemer> = redeemers
            .iter()
            .map(|r| {
                let tag = match format!("{:?}", r.purpose).to_lowercase().as_str() {
                    "spend" => RedeemerTag::Spend,
                    "mint" => RedeemerTag::Mint,
                    "cert" => RedeemerTag::Cert,
                    "reward" => RedeemerTag::Reward,
                    _ => RedeemerTag::Spend,
                };

                Redeemer {
                    tag,
                    index: r.tx_index as u64,
                    raw_cbor: vec![],
                    parsed: None,
                    ex_units: ExUnits {
                        mem: r.unit_mem.parse().unwrap_or(0),
                        steps: r.unit_steps.parse().unwrap_or(0),
                    },
                }
            })
            .collect();

        Ok(Transaction {
            hash: tx.hash.clone(),
            block: tx.block_height as u64,
            slot: tx.slot as u64,
            inputs,
            outputs,
            witnesses: Witnesses {
                redeemers: mapped_redeemers,
                ..Default::default()
            },
            metadata: None,
        })
    }

    /// Retry logic helper (static)
    async fn fetch_with_retry<T, F, Fut>(
        operation: F,
        max_retries: u32,
        retry_delay: Duration,
        context: &str,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = blockfrost::error::BlockfrostResult<T>>,
    {
        let mut last_error = None;
        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(
                    // Exponential backoff
                    (retry_delay.as_millis() * (1 << (attempt - 1))) as u64,
                );
                sleep(delay).await;
            }
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let err_str = format!("{:?}", e);
                    if err_str.contains("404") || err_str.contains("Invalid") {
                        return Err(Error::Blockfrost(format!("{} failed: {:?}", context, e)));
                    }
                    last_error = Some(e);
                }
            }
        }
        Err(Error::Blockfrost(format!(
            "{} failed after retries: {:?}",
            context, last_error
        )))
    }
    /// Static helper to parse datum from hex
    fn parse_datum_from_hex_static(hex: &str) -> Datum {
        match hex::decode(hex) {
            Ok(bytes) => Datum {
                hash: blake2b_256(&bytes),
                raw_cbor: bytes,
                parsed: None,
            },
            Err(e) => {
                tracing::warn!("Failed to decode datum hex: {}", e);
                Datum {
                    hash: "invalid".to_string(),
                    raw_cbor: vec![],
                    parsed: None,
                }
            }
        }
    }

    fn parse_datum_from_hex(&self, hex: &str) -> Datum {
        Self::parse_datum_from_hex_static(hex)
    }
}

#[async_trait]
impl DataSource for BlockfrostDataSource {
    async fn get_transaction(&self, tx_hash: &str) -> Result<Transaction> {
        Self::fetch_full_transaction(
            self.client.clone(),
            tx_hash.to_string(),
            self.max_retries,
            self.retry_delay,
        )
        .await
    }

    async fn get_transactions_by_address(
        &self,
        address: &str,
        params: QueryParams,
    ) -> Result<Vec<Transaction>> {
        tracing::debug!(
            "Fetching transactions for address {} from Blockfrost",
            address
        );

        // Get transaction references for this address
        // TODO: fetch multiple pages if needed
        let page_size = params.page_size.unwrap_or(100).min(100) as usize;
        let page = params.page.unwrap_or(1) as usize;
        let order = if params.order.as_deref() == Some("desc") {
            Order::Desc
        } else {
            Order::Asc
        };
        let pagination = Pagination::new(order, page, page_size);

        let tx_refs = self
            .execute_with_retry(|| async {
                self.client
                    .addresses_transactions(address, pagination)
                    .await
            })
            .await?;

        tracing::info!("Found {} transaction references for address", tx_refs.len());

        // Fetch full transaction data concurrently
        // Limit concurrency to avoid hitting rate limits too hard (10 parallel requests)
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));
        let mut tasks = Vec::new();

        for tx_ref in tx_refs {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let client_clone = self.client.clone();
            let tx_hash = tx_ref.tx_hash.clone();
            let max_retries = self.max_retries;
            let retry_delay = self.retry_delay;

            tasks.push(tokio::spawn(async move {
                let _permit = permit; // Hold permit
                Self::fetch_full_transaction(client_clone, tx_hash, max_retries, retry_delay).await
            }));
        }

        let mut transactions = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(tx)) => {
                    // Apply filters
                    let passes_filter = if let Some(from_slot) = params.from_slot {
                        tx.slot >= from_slot
                    } else {
                        true
                    } && if let Some(to_slot) = params.to_slot {
                        tx.slot <= to_slot
                    } else {
                        true
                    } && if let Some(from_block) = params.from_block {
                        tx.block >= from_block
                    } else {
                        true
                    } && if let Some(to_block) = params.to_block {
                        tx.block <= to_block
                    } else {
                        true
                    };

                    if passes_filter {
                        transactions.push(tx);
                    }
                }
                Ok(Err(e)) => tracing::warn!("Failed to fetch transaction: {}", e),
                Err(e) => tracing::warn!("Task join error: {}", e),
            }
        }

        tracing::info!("Successfully fetched {} transactions", transactions.len());
        Ok(transactions)
    }

    async fn get_script_utxos(&self, address: &str) -> Result<Vec<(UtxoRef, TxOutput)>> {
        tracing::debug!("Fetching UTXOs for address {} from Blockfrost", address);

        let pagination = Pagination::new(Order::Asc, 1, 100);

        let utxos = self
            .execute_with_retry(|| async { self.client.addresses_utxos(address, pagination).await })
            .await?;

        // Map Blockfrost UTXOs to our format
        let result = utxos
            .iter()
            .map(|utxo| {
                let utxo_ref = UtxoRef::new(utxo.tx_hash.clone(), utxo.output_index as u64);

                let amount: Vec<Asset> = utxo
                    .amount
                    .iter()
                    .map(|a| Asset {
                        unit: a.unit.clone(),
                        quantity: a.quantity.clone(),
                    })
                    .collect();

                // Handle datum
                let datum = if let Some(ref inline_datum) = utxo.inline_datum {
                    Some(self.parse_datum_from_hex(inline_datum))
                } else {
                    utxo.data_hash.as_ref().map(|data_hash| Datum {
                        hash: data_hash.clone(),
                        raw_cbor: vec![],
                        parsed: None,
                    })
                };

                let output = TxOutput {
                    address: address.to_string(),
                    amount,
                    datum,
                    script_ref: utxo.reference_script_hash.as_ref().map(|_hash| Script {
                        script_type: "Unknown".to_string(),
                        bytes: vec![],
                    }),
                };

                (utxo_ref, output)
            })
            .collect();

        Ok(result)
    }
}
