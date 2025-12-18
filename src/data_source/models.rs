//! Core data models for Cardano transactions
//!
//! This module defines the data structures representing Cardano transactions,
//! UTXOs, datums, redeemers, and related types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::parser::{ParsedDatum, ParsedRedeemer};

/// Unique reference to a UTXO
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UtxoRef {
    pub tx_hash: String, // TODO: use Bytes32 as in pallas::txbuilder:Input?
    pub output_index: u64,
}

impl UtxoRef {
    pub fn new(tx_hash: impl Into<String>, output_index: u64) -> Self {
        Self {
            tx_hash: tx_hash.into(),
            output_index,
        }
    }
}

impl fmt::Display for UtxoRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}#{}", self.tx_hash, self.output_index)
    }
}

/// Complete transaction data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction hash
    pub hash: String,

    /// Block number
    pub block: u64,

    /// Absolute slot number
    pub slot: u64,

    /// Transaction inputs
    pub inputs: Vec<TxInput>,

    /// Transaction outputs
    pub outputs: Vec<TxOutput>,

    /// Transaction witnesses
    pub witnesses: Witnesses,

    /// Transaction metadata
    pub metadata: Option<Metadata>,
}

/// Transaction input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    /// Reference to consumed UTXO
    pub utxo_ref: UtxoRef,

    /// Address of the consumed output
    pub address: Option<String>,

    /// Value of the consumed output
    pub amount: Option<Vec<Asset>>,
}

impl TxInput {
    pub fn as_utxo_ref(&self) -> &UtxoRef {
        &self.utxo_ref
    }
}

/// Transaction Output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    /// Output Address
    pub address: String,

    /// Output value (lovelace + native assets)
    pub amount: Vec<Asset>,

    /// Datum (inline or hash)
    pub datum: Option<Datum>,

    /// Reference script
    pub script_ref: Option<Script>,
}

/// Asset (ADA or native token)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    /// Asset unit (e.g. "lovelace" or "policy_id.asset_name")
    pub unit: String,
    pub quantity: String,
}

impl Asset {
    pub fn lovelace(amount: u64) -> Self {
        Self {
            unit: "lovelace".to_string(),
            quantity: amount.to_string(),
        }
    }

    pub fn native(policy_id: &str, asset_name: &str, quantity: u64) -> Self {
        Self {
            unit: format!("{}.{}", policy_id, asset_name),
            quantity: quantity.to_string(),
        }
    }
}

/// Datum (data attached to UTXO)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Datum {
    /// Datum hash
    pub hash: String,

    /// Raw CBOR bytes
    pub raw_cbor: Vec<u8>,

    /// Parsed PlutusData
    #[serde(skip)]
    pub parsed: Option<ParsedDatum>,
}

/// Script reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    /// Script type (PlutusV1, PlutusV2, PlutusV3, NativeScript)
    pub script_type: String,

    pub bytes: Vec<u8>,
}

/// Transaction witnesses
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Witnesses {
    /// Plutus datums in witness set
    pub datums: Vec<Datum>,

    /// Plutus redeemers
    pub redeemers: Vec<Redeemer>,

    /// Plutus scripts
    pub scripts: Vec<Script>,

    /// Other witness data (signatures, etc.)
    pub other: HashMap<String, serde_json::Value>,
}

/// Redeemer (action data for script execution)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redeemer {
    /// Redeemer tag (Spend, Mint, Cert, Reward)
    pub tag: RedeemerTag,

    /// Index within the tag category
    pub index: u64,

    /// Raw CBOR bytes
    pub raw_cbor: Vec<u8>,

    /// Parsed PlutusData (populated by parser)
    #[serde(skip)]
    pub parsed: Option<ParsedRedeemer>,

    /// Execution units
    pub ex_units: ExUnits,
}

/// Redeemer tag
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedeemerTag {
    Spend,
    Mint,
    Cert,
    Reward,
}

/// Execution units (memory and CPU steps)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExUnits {
    pub mem: u64,
    pub steps: u64,
}

/// Transaction metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// Metadata as JSON
    pub json: serde_json::Value,
}

/// Query parameters for fetching transactions
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct QueryParams {
    /// Page number (for pagination)
    pub page: Option<u32>,

    /// Page size (number of results per page)
    pub page_size: Option<u32>,

    /// Filter by minimum slot
    pub from_slot: Option<u64>,

    /// Filter by maximum slot
    pub to_slot: Option<u64>,

    /// Filter by minimum block
    pub from_block: Option<u64>,

    /// Filter by maximum block
    pub to_block: Option<u64>,

    /// Order (asc, desc)
    pub order: Option<String>,

    /// Max number of transactions to return
    pub limit: Option<usize>,
}

impl QueryParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }

    pub fn page_size(mut self, size: u32) -> Self {
        self.page_size = Some(size);
        self
    }

    pub fn from_slot(mut self, slot: u64) -> Self {
        self.from_slot = Some(slot);
        self
    }

    pub fn to_slot(mut self, slot: u64) -> Self {
        self.to_slot = Some(slot);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utxo_ref_display() {
        let utxo = UtxoRef::new("abc123def", 5);
        assert_eq!(utxo.to_string(), "abc123def#5");
    }

    #[test]
    fn test_utxo_ref_equality() {
        let utxo1 = UtxoRef::new("abc", 0);
        let utxo2 = UtxoRef::new("abc", 0);
        let utxo3 = UtxoRef::new("abc", 1);
        let utxo4 = UtxoRef::new("def", 0);

        assert_eq!(utxo1, utxo2);
        assert_ne!(utxo1, utxo3);
        assert_ne!(utxo1, utxo4);
    }

    #[test]
    fn test_utxo_ref_hash() {
        use std::collections::HashSet;

        let utxo1 = UtxoRef::new("abc", 0);
        let utxo2 = UtxoRef::new("abc", 0);
        let utxo3 = UtxoRef::new("def", 1);

        let mut set = HashSet::new();
        set.insert(utxo1.clone());
        set.insert(utxo2); // Should not increase set size
        set.insert(utxo3);

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_datum_serialization() {
        let datum = Datum {
            hash: "test_hash".to_string(),
            raw_cbor: vec![0xd8, 0x79],
            parsed: None,
        };

        let json = serde_json::to_string(&datum).unwrap();
        let deserialized: Datum = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.hash, "test_hash");
        assert_eq!(deserialized.raw_cbor, vec![0xd8, 0x79]);
        assert!(deserialized.parsed.is_none());
    }

    #[test]
    fn test_transaction_serialization() {
        let tx = Transaction {
            hash: "tx_test".to_string(),
            block: 42,
            slot: 1337,
            inputs: vec![],
            outputs: vec![TxOutput {
                address: "addr1".to_string(),
                amount: vec![Asset::lovelace(1000000)],
                datum: None,
                script_ref: None,
            }],
            witnesses: Witnesses::default(),
            metadata: None,
        };

        let json = serde_json::to_string(&tx).unwrap();
        let deserialized: Transaction = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.hash, "tx_test");
        assert_eq!(deserialized.outputs.len(), 1);
    }
}
