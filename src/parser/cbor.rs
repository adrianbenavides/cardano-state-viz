//! CBOR decoding and PlutusData types
//!
//! This module handles decoding of CBOR-encoded Plutus data structures
//! from Cardano transactions.

use crate::Result;
use chrono::DateTime;
use minicbor::decode::Decoder;
use serde::{Deserialize, Serialize};

/// PlutusData representation
///
/// Represents the on-chain data structures used in Plutus smart contracts.
/// Maps to the PlutusData type from the Plutus specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlutusData {
    /// Constructor with tag and fields (e.g., datums, redeemers)
    Constr { tag: u64, fields: Vec<PlutusData> },
    /// Map of key-value pairs
    Map(Vec<(PlutusData, PlutusData)>),
    /// List of PlutusData values
    List(Vec<PlutusData>),
    /// Arbitrary precision integer
    Integer(i128),
    /// Byte string
    Bytes(Vec<u8>),
}

use minicbor::encode::Encoder;

/// Encode PlutusData to CBOR bytes
pub fn encode_plutus_data(data: &PlutusData) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        // By using &mut buffer, we rely on std::io::Write implementation for Vec<u8>
        let mut encoder = Encoder::new(&mut buffer);
        encode_plutus_data_recursive(&mut encoder, data)
            .map_err(|e| crate::Error::CborDecode(format!("Failed to encode PlutusData: {}", e)))?;
    }
    Ok(buffer)
}

fn encode_plutus_data_recursive<W: minicbor::encode::Write>(
    encoder: &mut Encoder<W>,
    data: &PlutusData,
) -> std::result::Result<(), minicbor::encode::Error<W::Error>> {
    match data {
        PlutusData::Integer(n) => {
            encoder.i64(*n as i64)?;
        }
        PlutusData::Bytes(b) => {
            encoder.bytes(b)?;
        }
        PlutusData::List(items) => {
            encoder.begin_array()?;
            for item in items {
                encode_plutus_data_recursive(encoder, item)?;
            }
            encoder.end()?;
        }
        PlutusData::Map(pairs) => {
            encoder.begin_map()?;
            for (k, v) in pairs {
                encode_plutus_data_recursive(encoder, k)?;
                encode_plutus_data_recursive(encoder, v)?;
            }
            encoder.end()?;
        }
        PlutusData::Constr { tag, fields } => {
            let cbor_tag = if *tag <= 6 {
                121 + tag
            } else if *tag <= 127 {
                1280 + (tag - 7)
            } else {
                102
            };

            encoder.tag(minicbor::data::Tag::new(cbor_tag))?;

            encoder.begin_array()?;
            for field in fields {
                encode_plutus_data_recursive(encoder, field)?;
            }
            encoder.end()?;
        }
    }
    Ok(())
}

/// Decode CBOR bytes to PlutusData
///
/// Uses minicbor to decode CBOR-encoded PlutusData structures.
/// The encoding follows the Plutus data specification from CIP-42.
pub fn decode_plutus_data(cbor: &[u8]) -> Result<PlutusData> {
    let mut decoder = Decoder::new(cbor);
    decode_plutus_data_recursive(&mut decoder)
        .map_err(|e| crate::Error::CborDecode(format!("Failed to decode PlutusData: {}", e)))
}

/// Recursively decode PlutusData from CBOR
fn decode_plutus_data_recursive(decoder: &mut Decoder) -> Result<PlutusData> {
    use minicbor::data::Type;

    let data_type = decoder
        .datatype()
        .map_err(|e| crate::Error::CborDecode(format!("Failed to get datatype: {}", e)))?;

    match data_type {
        // Integer (positive or negative)
        Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::I8
        | Type::I16
        | Type::I32
        | Type::I64 => {
            let value: i128 = decoder
                .i64()
                .map_err(|e| crate::Error::CborDecode(format!("Failed to decode integer: {}", e)))?
                as i128;
            Ok(PlutusData::Integer(value))
        }

        // Byte string
        Type::Bytes => {
            let bytes: Vec<u8> = decoder
                .bytes()
                .map_err(|e| crate::Error::CborDecode(format!("Failed to decode bytes: {}", e)))?
                .to_vec();
            Ok(PlutusData::Bytes(bytes))
        }

        // Array (List or Constr alternative 0-6)
        Type::Array | Type::ArrayIndef => {
            let len = decoder
                .array()
                .map_err(|e| crate::Error::CborDecode(format!("Failed to decode array: {}", e)))?;

            let mut items = Vec::new();
            if let Some(len) = len {
                for _ in 0..len {
                    items.push(decode_plutus_data_recursive(decoder)?);
                }
            } else {
                // Indefinite length array
                loop {
                    if decoder.datatype().map_err(|e| {
                        crate::Error::CborDecode(format!("Failed to get datatype: {}", e))
                    })? == Type::Break
                    {
                        decoder.skip().map_err(|e| {
                            crate::Error::CborDecode(format!("Failed to skip break: {}", e))
                        })?;
                        break;
                    }
                    items.push(decode_plutus_data_recursive(decoder)?);
                }
            }
            Ok(PlutusData::List(items))
        }

        // Map
        Type::Map | Type::MapIndef => {
            let len = decoder
                .map()
                .map_err(|e| crate::Error::CborDecode(format!("Failed to decode map: {}", e)))?;

            let mut pairs = Vec::new();
            if let Some(len) = len {
                for _ in 0..len {
                    let key = decode_plutus_data_recursive(decoder)?;
                    let value = decode_plutus_data_recursive(decoder)?;
                    pairs.push((key, value));
                }
            } else {
                // Indefinite length map
                loop {
                    if decoder.datatype().map_err(|e| {
                        crate::Error::CborDecode(format!("Failed to get datatype: {}", e))
                    })? == Type::Break
                    {
                        decoder.skip().map_err(|e| {
                            crate::Error::CborDecode(format!("Failed to skip break: {}", e))
                        })?;
                        break;
                    }
                    let key = decode_plutus_data_recursive(decoder)?;
                    let value = decode_plutus_data_recursive(decoder)?;
                    pairs.push((key, value));
                }
            }
            Ok(PlutusData::Map(pairs))
        }

        // Tag (Constructor with alternative >= 7)
        Type::Tag => {
            let tag = decoder
                .tag()
                .map_err(|e| crate::Error::CborDecode(format!("Failed to decode tag: {}", e)))?;

            // Tag 121-127 are used for Constr 0-6 (compact encoding)
            // Tag 1280-1400 are used for Constr 7-127 (general encoding)
            let tag_value = tag.as_u64();
            if (121..=127).contains(&tag_value) {
                let constr_tag = tag_value - 121;
                let fields =
                    if let PlutusData::List(fields) = decode_plutus_data_recursive(decoder)? {
                        fields
                    } else {
                        return Err(crate::Error::CborDecode(
                            "Expected list for constructor fields".to_string(),
                        ));
                    };
                Ok(PlutusData::Constr {
                    tag: constr_tag,
                    fields,
                })
            } else if (1280..=1400).contains(&tag_value) {
                let constr_tag = tag_value - 1280 + 7;
                let fields =
                    if let PlutusData::List(fields) = decode_plutus_data_recursive(decoder)? {
                        fields
                    } else {
                        return Err(crate::Error::CborDecode(
                            "Expected list for constructor fields".to_string(),
                        ));
                    };
                Ok(PlutusData::Constr {
                    tag: constr_tag,
                    fields,
                })
            } else {
                Err(crate::Error::CborDecode(format!(
                    "Unsupported CBOR tag for PlutusData: {}",
                    tag_value
                )))
            }
        }

        _ => Err(crate::Error::CborDecode(format!(
            "Unsupported CBOR type: {:?}",
            data_type
        ))),
    }
}

impl PlutusData {
    /// Convert to human-readable string with heuristics
    ///
    /// Attempts to interpret common patterns:
    /// - 28-byte bytestrings as public key hashes
    /// - Large integers as POSIX timestamps
    /// - Small integers as booleans (0 = false, 1 = true)
    pub fn to_human_readable(&self) -> String {
        self.to_string()
    }

    fn format_recursive(&self, f: &mut std::fmt::Formatter<'_>, depth: usize) -> std::fmt::Result {
        if depth > 10 {
            return write!(f, "...");
        }

        match self {
            PlutusData::Bytes(b) if b.len() == 28 => {
                write!(f, "PubKeyHash({})", hex::encode(b))
            }
            PlutusData::Bytes(b) if b.len() == 32 => {
                write!(f, "Hash({})", hex::encode(b))
            }
            PlutusData::Bytes(b) => {
                // Try to interpret as ASCII/UTF-8 string
                if let Ok(s) = std::str::from_utf8(b)
                    && s.chars()
                        .all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
                {
                    return write!(f, "\"{}\"", s);
                }
                write!(f, "0x{}", hex::encode(b))
            }
            PlutusData::Integer(n) if *n == 0 || *n == 1 => {
                write!(f, "{}", if *n == 1 { "true" } else { "false" })
            }
            PlutusData::Integer(n) => {
                // Try to parse as timestamp (seconds since epoch)
                // Timestamps in Cardano are usually > 1,500,000,000
                if *n > 1_500_000_000
                    && *n < 2_500_000_000
                    && let Some(datetime) = DateTime::from_timestamp(*n as i64, 0)
                {
                    return write!(f, "{}", datetime.format("%Y-%m-%d %H:%M:%S UTC"));
                }
                write!(f, "{}", n)
            }
            PlutusData::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    item.format_recursive(f, depth + 1)?;
                }
                write!(f, "]")
            }
            PlutusData::Map(pairs) => {
                write!(f, "{{")?;
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    k.format_recursive(f, depth + 1)?;
                    write!(f, ": ")?;
                    v.format_recursive(f, depth + 1)?;
                }
                write!(f, "}}")
            }
            PlutusData::Constr { tag, fields } => {
                write!(f, "Constr({}", tag)?;
                if !fields.is_empty() {
                    write!(f, ", [")?;
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        field.format_recursive(f, depth + 1)?;
                    }
                    write!(f, "]")?;
                }
                write!(f, ")")
            }
        }
    }

    /// Get the constructor tag if this is a Constr
    pub fn as_constr_tag(&self) -> Option<u64> {
        match self {
            PlutusData::Constr { tag, .. } => Some(*tag),
            _ => None,
        }
    }

    /// Get the fields if this is a Constr
    pub fn as_constr_fields(&self) -> Option<&[PlutusData]> {
        match self {
            PlutusData::Constr { fields, .. } => Some(fields),
            _ => None,
        }
    }

    /// Get bytes if this is a Bytes variant
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            PlutusData::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Get integer if this is an Integer variant
    pub fn as_integer(&self) -> Option<i128> {
        match self {
            PlutusData::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// Get list if this is a List variant
    pub fn as_list(&self) -> Option<&[PlutusData]> {
        match self {
            PlutusData::List(items) => Some(items),
            _ => None,
        }
    }

    /// Get map if this is a Map variant
    pub fn as_map(&self) -> Option<&[(PlutusData, PlutusData)]> {
        match self {
            PlutusData::Map(pairs) => Some(pairs),
            _ => None,
        }
    }
}

impl std::fmt::Display for PlutusData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format_recursive(f, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_integer() {
        // CBOR encoding of integer 42
        let cbor = vec![0x18, 0x2a];
        let result = decode_plutus_data(&cbor).unwrap();
        assert_eq!(result, PlutusData::Integer(42));
    }

    #[test]
    fn test_decode_bytes() {
        // CBOR encoding of byte string "hello"
        let cbor = vec![0x45, 0x68, 0x65, 0x6c, 0x6c, 0x6f];
        let result = decode_plutus_data(&cbor).unwrap();
        assert_eq!(result, PlutusData::Bytes(b"hello".to_vec()));
    }

    #[test]
    fn test_decode_list() {
        // CBOR encoding of [1, 2, 3]
        let cbor = vec![0x83, 0x01, 0x02, 0x03];
        let result = decode_plutus_data(&cbor).unwrap();
        assert_eq!(
            result,
            PlutusData::List(vec![
                PlutusData::Integer(1),
                PlutusData::Integer(2),
                PlutusData::Integer(3),
            ])
        );
    }

    #[test]
    fn test_to_human_readable_pubkeyhash() {
        let bytes = vec![0xaa; 28];
        let data = PlutusData::Bytes(bytes.clone());
        let readable = data.to_human_readable();
        assert!(readable.starts_with("PubKeyHash("));
    }

    #[test]
    fn test_to_human_readable_timestamp() {
        let timestamp = 1705334400i128; // 2024-01-15 12:00:00 UTC
        let data = PlutusData::Integer(timestamp);
        let readable = data.to_human_readable();
        assert!(readable.contains("2024-01-15"));
    }

    #[test]
    fn test_to_human_readable_bool() {
        let zero = PlutusData::Integer(0);
        assert!(zero.to_human_readable().contains("false"));

        let one = PlutusData::Integer(1);
        assert!(one.to_human_readable().contains("true"));
    }

    #[test]
    fn test_helper_methods() {
        let constr = PlutusData::Constr {
            tag: 0,
            fields: vec![PlutusData::Integer(42)],
        };
        assert_eq!(constr.as_constr_tag(), Some(0));
        assert_eq!(constr.as_constr_fields().unwrap().len(), 1);

        let bytes_data = PlutusData::Bytes(vec![1, 2, 3]);
        assert_eq!(bytes_data.as_bytes(), Some(&[1, 2, 3][..]));

        let int_data = PlutusData::Integer(42);
        assert_eq!(int_data.as_integer(), Some(42));
    }

    #[test]
    fn test_decode_indefinite_array() {
        // Construct CBOR indefinite array: [ 1, 2, Break ]
        // 0x9f (Start Indefinite Array)
        // 0x01 (Integer 1)
        // 0x02 (Integer 2)
        // 0xff (Break)
        let cbor = vec![0x9f, 0x01, 0x02, 0xff];

        let result = decode_plutus_data(&cbor);

        match result {
            Ok(data) => assert_eq!(
                data,
                PlutusData::List(vec![PlutusData::Integer(1), PlutusData::Integer(2),])
            ),
            Err(e) => panic!("Failed to decode indefinite array: {}", e),
        }
    }
}
