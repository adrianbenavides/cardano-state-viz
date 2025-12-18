//! Output formatting module
//!
//! This module handles formatting transactions and datums for different output formats.

use crate::{
    data_source::{Datum, Transaction},
    Result,
};
use serde_json::json;

/// Output transactions and datums as JSON
pub fn output_json(
    w: &mut impl std::io::Write,
    transactions: &[Transaction],
    datums: &[(String, usize, Datum)],
) -> Result<()> {
    let output = json!({
        "summary": {
            "total_transactions": transactions.len(),
            "total_datums": datums.len(),
        },
        "transactions": transactions.iter().map(|tx| {
            json!({
                "hash": tx.hash,
                "block": tx.block,
                "slot": tx.slot,
                "inputs": tx.inputs.len(),
                "outputs": tx.outputs.len(),
            })
        }).collect::<Vec<_>>(),
        "datums": datums.iter().map(|(tx_hash, output_idx, datum)| {
            json!({
                "transaction": tx_hash,
                "output_index": output_idx,
                "hash": datum.hash,
                "parsed": datum.parsed.as_ref().map(|pd| pd.raw.to_human_readable()),
            })
        }).collect::<Vec<_>>(),
    });

    serde_json::to_writer_pretty(&mut *w, &output)?;
    writeln!(w)?; // Add trailing newline
    Ok(())
}

/// Output transactions and datums as text table
pub fn output_table(
    w: &mut impl std::io::Write,
    transactions: &[Transaction],
    datums: &[(String, usize, Datum)],
) -> Result<()> {
    writeln!(w, "Cardano State Visualization - Analysis Results")?;
    writeln!(w, "{}", "=".repeat(80))?;
    writeln!(w)?;

    writeln!(w, "Summary:")?;
    writeln!(w, "  Total Transactions: {}", transactions.len())?;
    writeln!(w, "  Total Datums:       {}", datums.len())?;
    writeln!(w)?;

    if !transactions.is_empty() {
        writeln!(w, "Transactions:")?;
        writeln!(w, "{:-<80}", "")?;
        writeln!(
            w,
            "{:<16} {:>8} {:>10} {:>8} {:>8}",
            "TX Hash", "Block", "Slot", "Inputs", "Outputs"
        )?;
        writeln!(w, "{:-<80}", "")?;

        for tx in transactions {
            let hash_short = if tx.hash.len() > 14 {
                format!("{}...", &tx.hash[..11])
            } else {
                tx.hash.clone()
            };

            writeln!(
                w,
                "{:<16} {:>8} {:>10} {:>8} {:>8}",
                hash_short,
                tx.block,
                tx.slot,
                tx.inputs.len(),
                tx.outputs.len()
            )?;
        }
        writeln!(w)?;
    }

    if !datums.is_empty() {
        writeln!(w, "Datums:")?;
        writeln!(w, "{:-<120}", "")?;
        writeln!(w, "{:<16} {:>5} {:<97}", "TX Hash", "Out#", "Parsed Data")?;
        writeln!(w, "{:-<120}", "")?;

        for (tx_hash, output_idx, datum) in datums {
            let hash_short = if tx_hash.len() > 14 {
                format!("{}...", &tx_hash[..11])
            } else {
                tx_hash.clone()
            };

            let parsed_str = datum
                .parsed
                .as_ref()
                .map(|pd| {
                    let s = pd.raw.to_human_readable();
                    if s.len() > 95 {
                        format!("{}...", &s[..92])
                    } else {
                        s
                    }
                })
                .unwrap_or_else(|| "Not parsed".to_string());

            writeln!(w, "{:<16} {:>5} {:<97}", hash_short, output_idx, parsed_str)?;
        }
        writeln!(w)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::{Asset, TxInput, TxOutput, UtxoRef, Witnesses};
    use crate::parser::PlutusData;

    fn create_test_transaction() -> Transaction {
        Transaction {
            hash: "tx_test_123".to_string(),
            block: 100,
            slot: 1000,
            inputs: vec![TxInput {
                utxo_ref: UtxoRef::new("prev_tx", 0),
                address: Some("addr1".to_string()),
                amount: Some(vec![Asset::lovelace(1_000_000)]),
            }],
            outputs: vec![TxOutput {
                address: "addr2".to_string(),
                amount: vec![Asset::lovelace(500_000)],
                datum: None,
                script_ref: None,
            }],
            witnesses: Witnesses::default(),
            metadata: None,
        }
    }

    fn create_test_datum() -> Datum {
        let mut datum = Datum {
            hash: "datum_hash_123".to_string(),
            raw_cbor: vec![0x18, 0x2a],
            parsed: None,
        };
        datum.parsed = Some(crate::parser::ParsedDatum {
            raw: PlutusData::Integer(42),
            fields: std::collections::HashMap::new(),
        });
        datum
    }

    #[test]
    fn test_output_json() {
        let tx = create_test_transaction();
        let datum = create_test_datum();
        let datums = vec![("tx_test_123".to_string(), 0, datum)];

        let mut output = Vec::new();
        let result = output_json(&mut output, &[tx], &datums);
        assert!(result.is_ok());
    }

    #[test]
    fn test_output_table() {
        let tx = create_test_transaction();
        let datum = create_test_datum();
        let datums = vec![("tx_test_123".to_string(), 0, datum)];

        let mut output = Vec::new();
        let result = output_table(&mut output, &[tx], &datums);
        assert!(result.is_ok());
    }
}
