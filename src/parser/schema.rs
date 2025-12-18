//! Schema-based parsing

use crate::{
    Result,
    parser::{ParsedDatum, ParsedRedeemer, Parser},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use std::collections::HashMap;

/// Contract schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractSchema {
    pub contract: ContractInfo,
    pub datum: DatumSchema,
    pub redeemer: Vec<RedeemerSchema>,
    #[serde(default)]
    pub states: HashMap<String, StateRule>,
    #[serde(default)]
    pub transitions: HashMap<String, TransitionStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRule {
    pub rule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionStyle {
    pub label: Option<String>,
    pub color: Option<String>,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInfo {
    pub name: String,
    pub description: Option<String>,
    pub script_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumSchema {
    #[serde(rename = "type")]
    pub datum_type: String,
    #[serde(default)]
    pub constructor_index: u64,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub desc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemerSchema {
    pub name: String,
    pub constructor_index: u64,
}

impl ContractSchema {
    pub fn from_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let contents = std::fs::read_to_string(&path)?;
        let schema: ContractSchema = toml::from_str(&contents)?;
        Ok(schema)
    }
}

/// Schema-based parser
pub struct SchemaParser {
    pub schema: ContractSchema,
}

impl SchemaParser {
    pub fn new(schema: ContractSchema) -> Self {
        Self { schema }
    }

    /// Classify a state based on schema rules
    pub fn classify_state(
        &self,
        state: &crate::state_machine::State,
    ) -> Option<crate::state_machine::StateClass> {
        // Iterate over rules in specific order if possible, or just check all
        // Priority: Terminal > Initial > Locked > Active > Unknown

        let rules = &self.schema.states;

        // Check terminal
        if let Some(rule) = rules.get("terminal")
            && self.evaluate_rule(&rule.rule, state)
        {
            return Some(crate::state_machine::StateClass::Completed); // Or Failed depending on context
        }

        // Check initial
        if let Some(rule) = rules.get("initial")
            && self.evaluate_rule(&rule.rule, state)
        {
            return Some(crate::state_machine::StateClass::Initial);
        }

        // Check locked
        if let Some(rule) = rules.get("locked")
            && self.evaluate_rule(&rule.rule, state)
        {
            return Some(crate::state_machine::StateClass::Locked);
        }

        // Check active
        if let Some(rule) = rules.get("active")
            && self.evaluate_rule(&rule.rule, state)
        {
            return Some(crate::state_machine::StateClass::Active);
        }

        None
    }

    /// Label a transition based on schema styles
    pub fn label_transition(&self, redeemer_name: &str) -> Option<String> {
        self.schema
            .transitions
            .get(redeemer_name)
            .and_then(|style| style.label.clone())
    }

    /// Evaluate a classification rule against a state
    fn evaluate_rule(&self, rule: &str, state: &crate::state_machine::State) -> bool {
        match rule {
            "new_utxo" => {
                // Initial state check - usually means no incoming transitions in graph context
                // But here we only have state. We might need graph context or rely on metadata
                // For now, heuristic: block 0 or specific marker?
                // Actually, "new_utxo" implies it wasn't created by the script (no parent in graph)
                // This is hard to evaluate on isolated state without graph context.
                // Let's assume it matches if classification is already Initial (from graph topology)
                state.metadata.classification == crate::state_machine::StateClass::Initial
            }
            "utxo_spent" => {
                // Terminal state check
                state.metadata.classification == crate::state_machine::StateClass::Completed
            }
            "always" => true,
            "never" => false,
            _ => {
                // Parse complex rules: "datum.field op value"
                // Simple parser for demonstration
                if rule.starts_with("datum.") {
                    let parts: Vec<&str> = rule.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let field_path = parts[0]; // datum.unlock_time
                        let op = parts[1]; // >
                        let val_str = parts[2]; // current_time or number

                        if let Some(field_name) = field_path.strip_prefix("datum.")
                            && let Some(datum) = &state.datum
                            && let Some(parsed) = &datum.parsed
                            && let Some(field_val) = parsed.fields.get(field_name)
                        {
                            // Compare
                            // Handle "current_time"
                            let target_val = if val_str == "current_time" {
                                // Mock current time - strictly this should come from config/context
                                // For now use a fixed future timestamp or actual current time
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as i128
                            } else {
                                val_str.parse::<i128>().unwrap_or(0)
                            };

                            let current_val = field_val.parse::<i128>().unwrap_or(0);

                            return match op {
                                ">" => current_val > target_val,
                                "<" => current_val < target_val,
                                ">=" => current_val >= target_val,
                                "<=" => current_val <= target_val,
                                "==" => current_val == target_val,
                                "!=" => current_val != target_val,
                                _ => false,
                            };
                        }
                    }
                }
                false
            }
        }
    }
}

impl Parser for SchemaParser {
    fn parse_datum(&self, raw: &[u8]) -> Result<ParsedDatum> {
        let raw_data = crate::parser::cbor::decode_plutus_data(raw)?;
        let mut fields = HashMap::new();

        // Check if schema matches
        // Currently only supporting top-level Constr matches
        if self.schema.datum.datum_type == "constr"
            && let crate::parser::PlutusData::Constr {
                tag,
                fields: data_fields,
            } = &raw_data
        {
            // Check constructor tag (index)
            if *tag == self.schema.datum.constructor_index {
                // Match fields
                for (i, field_def) in self.schema.datum.fields.iter().enumerate() {
                    if let Some(val) = data_fields.get(i) {
                        let val_str = match field_def.field_type.as_str() {
                            "int" => val
                                .as_integer()
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| val.to_human_readable()),
                            "bytes" => val
                                .as_bytes()
                                .map(hex::encode)
                                .unwrap_or_else(|| val.to_human_readable()),
                            // Fallback for other types
                            _ => val.to_human_readable(),
                        };
                        fields.insert(field_def.name.clone(), val_str);
                    }
                }
            }
        }

        Ok(ParsedDatum {
            raw: raw_data,
            fields,
        })
    }

    fn parse_redeemer(&self, raw: &[u8]) -> Result<ParsedRedeemer> {
        let raw_data = crate::parser::cbor::decode_plutus_data(raw)?;
        let mut name = None;

        if let crate::parser::PlutusData::Constr { tag, .. } = &raw_data {
            // Find matching redeemer definition
            for redeemer_def in &self.schema.redeemer {
                if redeemer_def.constructor_index == *tag {
                    name = Some(redeemer_def.name.clone());
                    break;
                }
            }
        }

        Ok(ParsedRedeemer {
            raw: raw_data,
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::Encoder;
    use minicbor::encode::write::Cursor;

    fn create_test_schema() -> ContractSchema {
        ContractSchema {
            contract: ContractInfo {
                name: "Test".to_string(),
                description: None,
                script_address: "addr_test1".to_string(),
            },
            datum: DatumSchema {
                datum_type: "constr".to_string(),
                constructor_index: 0,
                fields: vec![
                    FieldDef {
                        name: "field_int".to_string(),
                        field_type: "int".to_string(),
                        desc: None,
                    },
                    FieldDef {
                        name: "field_bytes".to_string(),
                        field_type: "bytes".to_string(),
                        desc: None,
                    },
                ],
            },
            redeemer: vec![
                RedeemerSchema {
                    name: "Unlock".to_string(),
                    constructor_index: 0,
                },
                RedeemerSchema {
                    name: "Cancel".to_string(),
                    constructor_index: 1,
                },
            ],
            states: HashMap::new(),
            transitions: HashMap::new(),
        }
    }

    #[test]
    fn test_schema_parse_datum() {
        let schema = create_test_schema();
        let parser = SchemaParser::new(schema);

        // Encode datum: Constr(0, [Int(42), Bytes(vec![0xAA])])
        let mut buffer = [0u8; 128];
        let mut encoder = Encoder::new(Cursor::new(&mut buffer[..]));

        // Tag 121 corresponds to Constr 0 in compact encoding
        encoder.tag(minicbor::data::Tag::new(121)).unwrap();
        encoder.array(2).unwrap();
        encoder.i32(42).unwrap();
        encoder.bytes(&[0xAA]).unwrap();

        let len = encoder.writer().position();
        let parsed = parser
            .parse_datum(&buffer[..len])
            .expect("Failed to parse datum");

        assert_eq!(
            parsed.fields.get("field_int").map(|s| s.as_str()),
            Some("42")
        );
        assert_eq!(
            parsed.fields.get("field_bytes").map(|s| s.as_str()),
            Some("aa")
        );
    }

    #[test]
    fn test_schema_parse_redeemer() {
        let schema = create_test_schema();
        let parser = SchemaParser::new(schema);

        // Encode redeemer: Constr(1, []) -> "Cancel"
        // Tag 122 corresponds to Constr 1 in compact encoding
        let mut buffer = [0u8; 128];
        let mut encoder = Encoder::new(Cursor::new(&mut buffer[..]));

        encoder.tag(minicbor::data::Tag::new(122)).unwrap();
        encoder.array(0).unwrap(); // Empty fields

        let len = encoder.writer().position();
        let parsed = parser
            .parse_redeemer(&buffer[..len])
            .expect("Failed to parse redeemer");

        assert_eq!(parsed.name.as_deref(), Some("Cancel"));
    }
}
