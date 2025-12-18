//! Datum extraction and parsing
//!
//! This module handles extracting and parsing datums from transaction outputs.

use crate::{
    Result,
    data_source::{Datum, Transaction, TxOutput, Witnesses},
};

/// Datum extractor for extracting datums from transaction outputs
pub struct DatumExtractor {
    /// Whether to validate datum hashes
    validate_hashes: bool,
}

impl DatumExtractor {
    pub fn new() -> Self {
        Self {
            validate_hashes: true,
        }
    }

    pub fn without_validation() -> Self {
        Self {
            validate_hashes: false,
        }
    }

    /// Extract datum from transaction output
    ///
    /// Handles both:
    /// - Inline datums (Babbage era): Datum is directly embedded in the output
    /// - Datum hashes: Datum must be looked up in the transaction witnesses
    pub fn extract_datum(
        &self,
        output: &TxOutput,
        witnesses: Option<&Witnesses>,
    ) -> Result<Option<Datum>> {
        if let Some(datum) = &output.datum {
            // Datum is present (either inline or hash reference)
            if !datum.raw_cbor.is_empty() {
                // Inline datum - already has CBOR bytes
                Ok(Some(datum.clone()))
            } else if let Some(witnesses) = witnesses {
                // Datum hash - look up in witnesses
                self.lookup_datum_in_witnesses(&datum.hash, witnesses)
            } else {
                Err(crate::Error::Parser(format!(
                    "Datum hash {} provided but no witnesses available",
                    datum.hash
                )))
            }
        } else {
            Ok(None)
        }
    }

    /// Look up a datum by hash in the transaction witnesses
    fn lookup_datum_in_witnesses(
        &self,
        datum_hash: &str,
        witnesses: &Witnesses,
    ) -> Result<Option<Datum>> {
        for datum in &witnesses.datums {
            if datum.hash == datum_hash {
                if self.validate_hashes && !datum.raw_cbor.is_empty() {
                    // TODO: Validate that hash matches CBOR content
                    //  For now, trust the provided hash
                }
                return Ok(Some(datum.clone()));
            }
        }

        // Datum not found in witnesses
        Err(crate::Error::Parser(format!(
            "Datum with hash {} not found in transaction witnesses",
            datum_hash
        )))
    }

    /// Extract all datums from a transaction's outputs
    pub fn extract_all_datums(&self, transaction: &Transaction) -> Result<Vec<(usize, Datum)>> {
        let mut datums = Vec::new();

        for (idx, output) in transaction.outputs.iter().enumerate() {
            if let Some(datum) = self.extract_datum(output, Some(&transaction.witnesses))? {
                datums.push((idx, datum));
            }
        }

        Ok(datums)
    }

    /// Extract and parse datum from output
    pub fn extract_and_parse(
        &self,
        output: &TxOutput,
        witnesses: Option<&Witnesses>,
    ) -> Result<Option<(Datum, crate::parser::ParsedDatum)>> {
        if let Some(mut datum) = self.extract_datum(output, witnesses)? {
            // Parse the CBOR bytes
            let plutus_data = crate::parser::cbor::decode_plutus_data(&datum.raw_cbor)?;

            // Create ParsedDatum (generic)
            let parsed_datum = crate::parser::ParsedDatum {
                raw: plutus_data,
                fields: std::collections::HashMap::new(),
            };

            // Store parsed data in datum
            datum.parsed = Some(parsed_datum.clone());

            Ok(Some((datum, parsed_datum)))
        } else {
            Ok(None)
        }
    }
}

impl Default for DatumExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract datum from transaction output
///
/// This is a convenience wrapper around `DatumExtractor::extract_datum`.
pub fn extract_datum(output: &TxOutput, witnesses: Option<&Witnesses>) -> Result<Option<Datum>> {
    DatumExtractor::new().extract_datum(output, witnesses)
}

/// Validate that a datum hash matches its CBOR content
pub fn validate_datum_hash(datum: &Datum) -> Result<bool> {
    if datum.raw_cbor.is_empty() {
        return Ok(false);
    }

    // TODO: Implement proper hash validation

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::{Asset, Witnesses};
    use crate::parser::PlutusData;

    fn create_test_output_with_inline_datum() -> TxOutput {
        let datum = Datum {
            hash: "test_hash_123".to_string(),
            raw_cbor: vec![0x18, 0x2a], // CBOR encoding of integer 42
            parsed: None,
        };

        TxOutput {
            address: "addr_test1script".to_string(),
            amount: vec![Asset::lovelace(5_000_000)],
            datum: Some(datum),
            script_ref: None,
        }
    }

    fn create_test_output_with_datum_hash() -> TxOutput {
        let datum = Datum {
            hash: "datum_hash_456".to_string(),
            raw_cbor: vec![], // Empty - needs witness lookup
            parsed: None,
        };

        TxOutput {
            address: "addr_test1script".to_string(),
            amount: vec![Asset::lovelace(5_000_000)],
            datum: Some(datum),
            script_ref: None,
        }
    }

    fn create_test_witnesses() -> Witnesses {
        let datum = Datum {
            hash: "datum_hash_456".to_string(),
            raw_cbor: vec![0x01], // CBOR encoding of integer 1
            parsed: None,
        };

        Witnesses {
            datums: vec![datum],
            redeemers: vec![],
            scripts: vec![],
            other: Default::default(),
        }
    }

    #[test]
    fn test_extract_inline_datum() {
        let extractor = DatumExtractor::new();
        let output = create_test_output_with_inline_datum();

        let result = extractor.extract_datum(&output, None).unwrap();
        assert!(result.is_some());

        let datum = result.unwrap();
        assert_eq!(datum.hash, "test_hash_123");
        assert_eq!(datum.raw_cbor, vec![0x18, 0x2a]);
    }

    #[test]
    fn test_extract_datum_hash_with_witnesses() {
        let extractor = DatumExtractor::new();
        let output = create_test_output_with_datum_hash();
        let witnesses = create_test_witnesses();

        let result = extractor.extract_datum(&output, Some(&witnesses)).unwrap();
        assert!(result.is_some());

        let datum = result.unwrap();
        assert_eq!(datum.hash, "datum_hash_456");
        assert_eq!(datum.raw_cbor, vec![0x01]);
    }

    #[test]
    fn test_extract_datum_hash_without_witnesses() {
        let extractor = DatumExtractor::new();
        let output = create_test_output_with_datum_hash();

        let result = extractor.extract_datum(&output, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no witnesses"));
    }

    #[test]
    fn test_extract_datum_hash_not_in_witnesses() {
        let extractor = DatumExtractor::new();
        let output = create_test_output_with_datum_hash();
        let empty_witnesses = Witnesses::default();

        let result = extractor.extract_datum(&output, Some(&empty_witnesses));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_extract_and_parse_inline_datum() {
        let extractor = DatumExtractor::new();
        let output = create_test_output_with_inline_datum();

        let result = extractor.extract_and_parse(&output, None).unwrap();
        assert!(result.is_some());

        let (datum, parsed_datum) = result.unwrap();
        assert_eq!(datum.hash, "test_hash_123");
        assert_eq!(parsed_datum.raw, PlutusData::Integer(42));
    }

    #[test]
    fn test_extract_all_datums_from_transaction() {
        use crate::data_source::Transaction;

        let extractor = DatumExtractor::new();
        let tx = Transaction {
            hash: "tx_test".to_string(),
            block: 100,
            slot: 1000,
            inputs: vec![],
            outputs: vec![
                create_test_output_with_inline_datum(),
                TxOutput {
                    address: "addr_user".to_string(),
                    amount: vec![Asset::lovelace(1_000_000)],
                    datum: None,
                    script_ref: None,
                },
                create_test_output_with_datum_hash(),
            ],
            witnesses: create_test_witnesses(),
            metadata: None,
        };

        let datums = extractor.extract_all_datums(&tx).unwrap();
        assert_eq!(datums.len(), 2); // Two outputs have datums
        assert_eq!(datums[0].0, 0); // First output
        assert_eq!(datums[1].0, 2); // Third output
    }

    #[test]
    fn test_output_without_datum() {
        let extractor = DatumExtractor::new();
        let output = TxOutput {
            address: "addr_user".to_string(),
            amount: vec![Asset::lovelace(1_000_000)],
            datum: None,
            script_ref: None,
        };

        let result = extractor.extract_datum(&output, None).unwrap();
        assert!(result.is_none());
    }
}
