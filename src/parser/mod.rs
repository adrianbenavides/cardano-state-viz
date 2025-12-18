//! Parser module - CBOR decoding and datum/redeemer parsing

use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod cbor;
pub mod datum;
pub mod schema;
pub mod transaction;

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

/// Parser trait for datum/redeemer parsing
pub trait Parser {
    fn parse_datum(&self, raw: &[u8]) -> Result<ParsedDatum>;
    fn parse_redeemer(&self, raw: &[u8]) -> Result<ParsedRedeemer>;
}

/// Generic parser (no schema)
pub struct GenericParser;

impl Parser for GenericParser {
    fn parse_datum(&self, raw: &[u8]) -> Result<ParsedDatum> {
        let plutus_data = cbor::decode_plutus_data(raw)?;
        // TODO: Generate generic field names (field_0, field_1, ...)
        Ok(ParsedDatum {
            raw: plutus_data,
            fields: HashMap::new(),
        })
    }

    fn parse_redeemer(&self, raw: &[u8]) -> Result<ParsedRedeemer> {
        let plutus_data = cbor::decode_plutus_data(raw)?;
        Ok(ParsedRedeemer {
            raw: plutus_data,
            name: None,
        })
    }
}
