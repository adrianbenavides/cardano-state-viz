//! Parser module - CBOR decoding and datum/redeemer parsing

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod cbor;

// Re-export key types
pub use cbor::PlutusData;

/// Parsed datum with field labels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDatum {
    pub raw: PlutusData,
    pub fields: HashMap<String, String>,
}

/// Parsed redeemer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRedeemer {
    pub raw: PlutusData,
    pub name: Option<String>,
}
