//! Mock data source for testing and development
//!
//! Provides sample vesting contract transactions for testing and development.
//! The mock vesting contract locks tokens that can be unlocked over time.

use super::{
    Asset, DataSource, Datum, ExUnits, QueryParams, Redeemer, RedeemerTag, Transaction, TxInput,
    TxOutput, UtxoRef, Witnesses,
};
use crate::Result;
use crate::parser::PlutusData;
use async_trait::async_trait;

/// Mock script address for the vesting contract
pub const MOCK_SCRIPT_ADDRESS: &str = "mock";

/// Mock data source providing hardcoded sample transactions
pub struct MockDataSource {
    transactions: Vec<Transaction>,
    _script_address: String,
}

impl MockDataSource {
    pub fn new() -> Self {
        Self {
            transactions: Self::generate_sample_transactions(),
            _script_address: MOCK_SCRIPT_ADDRESS.to_string(),
        }
    }

    /// Generate sample vesting contract transactions
    ///
    /// Scenario: User locks 10,000 ADA that vests over 4 milestones:
    /// 1. Lock 10,000 ADA at slot 1000
    /// 2. Unlock 2,500 ADA at slot 2000 (25% vested)
    /// 3. Unlock 2,500 ADA at slot 3000 (50% vested)
    /// 4. Unlock 2,500 ADA at slot 4000 (75% vested)
    /// 5. Unlock final 2,500 ADA at slot 5000 (100% vested)
    fn generate_sample_transactions() -> Vec<Transaction> {
        let beneficiary = vec![0xaa; 28]; // Mock public key hash
        let total_amount: u64 = 10_000_000_000; // 10,000 ADA in lovelace
        let vesting_amount: u64 = 2_500_000_000; // 2,500 ADA per unlock

        vec![
            // Transaction 1: Lock tokens (initial state)
            Self::create_lock_tx(
                "0000000000000000000000000000000000000000000000000000000000000001",
                1000,
                100,
                &beneficiary,
                total_amount,
                1705334400, // Unix timestamp for unlock
            ),
            // Transaction 2: First unlock (25% vested)
            Self::create_unlock_tx(
                "0000000000000000000000000000000000000000000000000000000000000002",
                2000,
                200,
                "0000000000000000000000000000000000000000000000000000000000000001",
                0,
                &beneficiary,
                total_amount,
                vesting_amount,
                1705334400,
            ),
            // Transaction 3: Second unlock (50% vested)
            Self::create_unlock_tx(
                "0000000000000000000000000000000000000000000000000000000000000003",
                3000,
                300,
                "0000000000000000000000000000000000000000000000000000000000000002",
                1,
                &beneficiary,
                total_amount,
                vesting_amount * 2,
                1705334400,
            ),
            // Transaction 4: Third unlock (75% vested)
            Self::create_unlock_tx(
                "0000000000000000000000000000000000000000000000000000000000000004",
                4000,
                400,
                "0000000000000000000000000000000000000000000000000000000000000003",
                1,
                &beneficiary,
                total_amount,
                vesting_amount * 3,
                1705334400,
            ),
            // Transaction 5: Final unlock (100% vested - terminal state)
            Self::create_final_unlock_tx(
                "0000000000000000000000000000000000000000000000000000000000000005",
                5000,
                500,
                "0000000000000000000000000000000000000000000000000000000000000004",
                1,
                &beneficiary,
                total_amount,
                1705334400,
            ),
        ]
    }

    /// Create a lock transaction (initial state)
    fn create_lock_tx(
        tx_hash: &str,
        slot: u64,
        block: u64,
        beneficiary: &[u8],
        total_amount: u64,
        unlock_time: i64,
    ) -> Transaction {
        let datum = Self::create_vesting_datum(beneficiary, total_amount, 0, unlock_time);

        Transaction {
            hash: tx_hash.to_string(),
            block,
            slot,
            inputs: vec![TxInput {
                utxo_ref: UtxoRef::new("tx_user_wallet", 0),
                address: Some("addr_test1user_wallet_address".to_string()),
                amount: Some(vec![Asset::lovelace(total_amount + 2_000_000)]), // +2 ADA for fees
            }],
            outputs: vec![
                // Output 0: Locked UTXO at script address
                TxOutput {
                    address: MOCK_SCRIPT_ADDRESS.to_string(),
                    amount: vec![Asset::lovelace(total_amount)],
                    datum: Some(datum),
                    script_ref: None,
                },
                // Output 1: Change back to user
                TxOutput {
                    address: "addr_test1user_wallet_address".to_string(),
                    amount: vec![Asset::lovelace(2_000_000)],
                    datum: None,
                    script_ref: None,
                },
            ],
            witnesses: Witnesses::default(),
            metadata: None,
        }
    }

    /// Create an unlock transaction (intermediate state)
    #[allow(clippy::too_many_arguments)]
    fn create_unlock_tx(
        tx_hash: &str,
        slot: u64,
        block: u64,
        prev_tx: &str,
        prev_output_idx: u64,
        beneficiary: &[u8],
        total_amount: u64,
        already_vested: u64,
        unlock_time: i64,
    ) -> Transaction {
        let new_datum =
            Self::create_vesting_datum(beneficiary, total_amount, already_vested, unlock_time);
        let unlock_redeemer = Self::create_unlock_redeemer();
        let remaining = total_amount - already_vested;

        Transaction {
            hash: tx_hash.to_string(),
            block,
            slot,
            inputs: vec![TxInput {
                utxo_ref: UtxoRef::new(prev_tx, prev_output_idx),
                address: Some(MOCK_SCRIPT_ADDRESS.to_string()),
                amount: Some(vec![Asset::lovelace(
                    total_amount - (already_vested - 2_500_000_000),
                )]),
            }],
            outputs: vec![
                // Output 0: User receives vested amount
                TxOutput {
                    address: "addr_test1user_wallet_address".to_string(),
                    amount: vec![Asset::lovelace(2_500_000_000)],
                    datum: None,
                    script_ref: None,
                },
                // Output 1: Remaining locked at script
                TxOutput {
                    address: MOCK_SCRIPT_ADDRESS.to_string(),
                    amount: vec![Asset::lovelace(remaining)],
                    datum: Some(new_datum),
                    script_ref: None,
                },
            ],
            witnesses: Witnesses {
                redeemers: vec![unlock_redeemer],
                ..Default::default()
            },
            metadata: None,
        }
    }

    /// Create final unlock transaction (terminal state)
    #[allow(clippy::too_many_arguments)]
    fn create_final_unlock_tx(
        tx_hash: &str,
        slot: u64,
        block: u64,
        prev_tx: &str,
        prev_output_idx: u64,
        _beneficiary: &[u8],
        total_amount: u64,
        _unlock_time: i64,
    ) -> Transaction {
        let unlock_redeemer = Self::create_unlock_redeemer();
        let remaining = total_amount - (total_amount - 2_500_000_000);

        Transaction {
            hash: tx_hash.to_string(),
            block,
            slot,
            inputs: vec![TxInput {
                utxo_ref: UtxoRef::new(prev_tx, prev_output_idx),
                address: Some(MOCK_SCRIPT_ADDRESS.to_string()),
                amount: Some(vec![Asset::lovelace(remaining)]),
            }],
            outputs: vec![
                // Output 0: User receives final vested amount (no script output = terminal)
                TxOutput {
                    address: "addr_test1user_wallet_address".to_string(),
                    amount: vec![Asset::lovelace(remaining)],
                    datum: None,
                    script_ref: None,
                },
            ],
            witnesses: Witnesses {
                redeemers: vec![unlock_redeemer],
                ..Default::default()
            },
            metadata: None,
        }
    }

    /// Create vesting datum with PlutusData structure
    fn create_vesting_datum(
        beneficiary: &[u8],
        total_amount: u64,
        vested_amount: u64,
        unlock_time: i64,
    ) -> Datum {
        // Vesting datum structure (Constr 0):
        // Field 0: beneficiary (PubKeyHash - 28 bytes)
        // Field 1: total_amount (Integer)
        // Field 2: vested_amount (Integer)
        // Field 3: unlock_time (POSIXTime)
        let plutus_data = PlutusData::Constr {
            tag: 0,
            fields: vec![
                PlutusData::Bytes(beneficiary.to_vec()),
                PlutusData::Integer(total_amount as i128),
                PlutusData::Integer(vested_amount as i128),
                PlutusData::Integer(unlock_time as i128),
            ],
        };

        let raw_cbor = crate::parser::cbor::encode_plutus_data(&plutus_data).unwrap_or_default();

        Datum {
            hash: format!("datum_hash_{:016x}", total_amount ^ vested_amount),
            raw_cbor,
            parsed: Some(crate::parser::ParsedDatum {
                raw: plutus_data,
                fields: std::collections::HashMap::new(),
            }),
        }
    }

    /// Create unlock redeemer
    fn create_unlock_redeemer() -> Redeemer {
        // Redeemer for unlock action (Constr 0 for "Unlock")
        let plutus_data = PlutusData::Constr {
            tag: 0,
            fields: vec![],
        };

        let raw_cbor = crate::parser::cbor::encode_plutus_data(&plutus_data).unwrap_or_default();

        Redeemer {
            tag: RedeemerTag::Spend,
            index: 0,
            raw_cbor,
            parsed: Some(crate::parser::ParsedRedeemer {
                raw: plutus_data,
                name: None,
            }),
            ex_units: ExUnits {
                mem: 1_000_000,
                steps: 500_000_000,
            },
        }
    }
}

impl Default for MockDataSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DataSource for MockDataSource {
    async fn get_transaction(&self, tx_hash: &str) -> Result<Transaction> {
        self.transactions
            .iter()
            .find(|tx| tx.hash == tx_hash)
            .cloned()
            .ok_or_else(|| crate::Error::custom(format!("Transaction not found: {}", tx_hash)))
    }

    async fn get_transactions_by_address(
        &self,
        address: &str,
        _params: QueryParams,
    ) -> Result<Vec<Transaction>> {
        // Filter transactions that involve the given address
        let filtered: Vec<Transaction> = self
            .transactions
            .iter()
            .filter(|tx| {
                tx.outputs.iter().any(|out| out.address == address)
                    || tx
                        .inputs
                        .iter()
                        .any(|inp| inp.address.as_deref() == Some(address))
            })
            .cloned()
            .collect();

        Ok(filtered)
    }

    async fn get_script_utxos(&self, address: &str) -> Result<Vec<(UtxoRef, TxOutput)>> {
        let mut utxos = std::collections::HashMap::new();

        // Add all outputs at script address
        for tx in &self.transactions {
            for (idx, output) in tx.outputs.iter().enumerate() {
                if output.address == address {
                    let utxo_ref = UtxoRef::new(&tx.hash, idx as u64);
                    utxos.insert(utxo_ref, output.clone());
                }
            }
        }

        // Remove spent outputs
        for tx in &self.transactions {
            for input in &tx.inputs {
                if input.address.as_deref() == Some(address) {
                    utxos.remove(&input.utxo_ref);
                }
            }
        }

        Ok(utxos.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_data_source_transactions() {
        let source = MockDataSource::new();
        let txs = source
            .get_transactions_by_address(MOCK_SCRIPT_ADDRESS, QueryParams::default())
            .await
            .unwrap();

        assert_eq!(txs.len(), 5, "Should have 5 transactions");
        assert_eq!(
            txs[0].hash,
            "0000000000000000000000000000000000000000000000000000000000000001"
        );
        assert_eq!(
            txs[4].hash,
            "0000000000000000000000000000000000000000000000000000000000000005"
        );
    }

    #[tokio::test]
    async fn test_get_transaction_by_hash() {
        let source = MockDataSource::new();
        let tx = source
            .get_transaction("0000000000000000000000000000000000000000000000000000000000000001")
            .await
            .unwrap();

        assert_eq!(
            tx.hash,
            "0000000000000000000000000000000000000000000000000000000000000001"
        );
        assert_eq!(tx.block, 100);
        assert_eq!(tx.slot, 1000);
    }

    #[tokio::test]
    async fn test_get_script_utxos() {
        let source = MockDataSource::new();
        let utxos = source.get_script_utxos(MOCK_SCRIPT_ADDRESS).await.unwrap();

        // After all transactions, no UTXOs should remain (all unlocked)
        assert_eq!(utxos.len(), 0, "All tokens should be unlocked");
    }

    #[test]
    fn test_vesting_datum_structure() {
        let beneficiary = vec![0xaa; 28];
        let datum =
            MockDataSource::create_vesting_datum(&beneficiary, 10_000_000_000, 0, 1705334400);

        assert!(datum.parsed.is_some());
        if let Some(parsed) = datum.parsed {
            if let PlutusData::Constr { tag, fields } = parsed.raw {
                assert_eq!(tag, 0);
                assert_eq!(fields.len(), 4);
            } else {
                panic!("Expected Constr datum");
            }
        } else {
            panic!("Expected parsed datum");
        }
    }
}
