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
use crate::data_source::cache::DataSourceCache;
use crate::{Error, Result};
use async_trait::async_trait;
use blockfrost::{BlockFrostSettings, BlockfrostAPI, Order, Pagination};
use std::sync::Arc;
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
#[derive(Debug, Clone)] // Derived Clone to easily pass to tasks if needed, though client is cloneable
pub struct BlockfrostDataSource {
    client: BlockfrostAPI,
    max_retries: u32,
    retry_delay: Duration,
    cache: Option<Arc<DataSourceCache>>,
}

impl BlockfrostDataSource {
    /// Create a new Blockfrost data source
    pub fn new(api_key: String) -> Result<Self> {
        Ok(Self {
            client: BlockfrostAPI::new(&api_key, BlockFrostSettings::new()),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
            cache: None,
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

    /// Set cache
    pub fn with_cache(mut self, cache: DataSourceCache) -> Self {
        self.cache = Some(Arc::new(cache));
        self
    }

    /// Execute a Blockfrost API call with retry logic
    async fn execute_with_retry<F, T, Fut>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = blockfrost::error::BlockfrostResult<T>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                // 10 seconds + jitter, as per Blockfrost recommendations
                let delay =
                    Duration::from_secs(10) + Duration::from_millis(rand::random::<u64>() % 1000);
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
        cache: Option<Arc<DataSourceCache>>,
    ) -> Result<Transaction> {
        // Check cache first
        if let Some(ref c) = cache
            && let Some(tx) = c.get_transaction(&tx_hash).await
        {
            return Ok(tx);
        }

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

        let transaction = Transaction {
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
        };

        // Save to cache
        if let Some(ref c) = cache {
            c.save_transaction(&transaction).await;
        }

        Ok(transaction)
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
        Fut: Future<Output = blockfrost::error::BlockfrostResult<T>>,
    {
        let mut last_error = None;
        for attempt in 0..=max_retries {
            if attempt > 0 {
                // 10 seconds + jitter, as per Blockfrost recommendations
                let delay =
                    Duration::from_secs(10) + Duration::from_millis(rand::random::<u64>() % 1000);
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
            self.cache.clone(),
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
        let page_size = params.page_size.unwrap_or(100).min(1000) as usize;
        let mut page = params.page.unwrap_or(1) as usize;
        let fetch_all = params.page.is_none();
        let order = if params.order.as_deref() == Some("desc") {
            Order::Desc
        } else {
            Order::Asc
        };

        let mut tx_refs: Vec<serde_json::Value> = Vec::new();

        loop {
            // Check cache for this page
            let cache_key = if self.cache.is_some() {
                Some(DataSourceCache::cache_key_for(
                    address,
                    page,
                    page_size,
                    if matches!(order, Order::Desc) {
                        "desc"
                    } else {
                        "asc"
                    },
                ))
            } else {
                None
            };

            let page_refs: Vec<serde_json::Value> = if let Some(ref key) = cache_key
                && let Some(ref c) = self.cache
                && let Some(content) = c.get_text(key).await
            {
                tracing::info!(%page, %page_size, "Cache hit for address transactions page");
                serde_json::from_str(&content).unwrap_or_else(|e| {
                    tracing::warn!("Failed to deserialize cached page: {}", e);
                    vec![]
                })
            } else {
                vec![]
            };

            let page_refs = if !page_refs.is_empty() {
                page_refs
            } else {
                let pagination = Pagination::new(order, page, page_size);
                tracing::info!(%page, %page_size,
                    "Fetching address transactions from Blockfrost",
                );
                let fetched_refs = self
                    .execute_with_retry(|| async {
                        self.client
                            .addresses_transactions(address, pagination)
                            .await
                    })
                    .await?;

                // Convert to generic JSON values
                let json_refs: Vec<serde_json::Value> = match serde_json::to_value(&fetched_refs) {
                    Ok(serde_json::Value::Array(arr)) => arr,
                    Ok(_) => vec![], // Should be an array
                    Err(e) => {
                        tracing::warn!("Failed to serialize fetched refs to JSON: {}", e);
                        vec![]
                    }
                };

                // Cache the fetched page
                if let Some(ref key) = cache_key
                    && let Some(ref c) = self.cache
                    && let Ok(json) = serde_json::to_string(&json_refs)
                {
                    c.save_text(key, &json).await;
                }

                json_refs
            };

            let count = page_refs.len();
            tx_refs.extend(page_refs);

            if !fetch_all || count < page_size {
                break;
            }

            page += 1;
        }

        tracing::info!("Found {} transaction references for address", tx_refs.len());

        // Fetch full transaction data concurrently
        // Limit concurrency to avoid hitting rate limits too hard
        // (3 top-level requests * 3 internal requests each = 9 concurrent reqs)
        let semaphore = Arc::new(tokio::sync::Semaphore::new(3));
        let mut tasks = Vec::new();

        for tx_ref in tx_refs {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let client_clone = self.client.clone();

            // Extract hash from JSON value
            let tx_hash = tx_ref
                .get("tx_hash")
                .and_then(|h| h.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();

            if tx_hash.is_empty() {
                tracing::warn!("Found transaction ref without hash, skipping");
                continue;
            }

            let max_retries = self.max_retries;
            let retry_delay = self.retry_delay;
            let cache_clone = self.cache.clone();

            tasks.push(tokio::spawn(async move {
                let _permit = permit; // Hold permit
                Self::fetch_full_transaction(
                    client_clone,
                    tx_hash,
                    max_retries,
                    retry_delay,
                    cache_clone,
                )
                .await
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
