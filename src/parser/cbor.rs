//! CBOR decoding and PlutusData types
//!
//! This module handles decoding of CBOR-encoded Plutus data structures
//! from Cardano transactions.

use serde::{Deserialize, Serialize};

/// PlutusData representation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlutusData {}
