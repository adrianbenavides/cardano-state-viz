//! State representation

use crate::data_source::{Datum, TxOutput, UtxoRef};
use serde::{Deserialize, Serialize};

pub type StateId = String;

/// A state in the state machine (represents a UTXO)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub id: StateId,
    pub utxo_ref: UtxoRef,
    pub datum: Option<Datum>,
    pub output: TxOutput,
    pub block: u64,
    pub slot: u64,
    pub tx_hash: String,
    pub metadata: StateMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMetadata {
    pub classification: StateClass,
    pub label: Option<String>,
    pub color: Option<String>,
}

/// State classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StateClass {
    Initial,
    Active,
    Locked,
    Completed,
    Failed,
    #[default]
    Unknown,
}

impl StateClass {
    pub fn color(&self) -> &'static str {
        match self {
            StateClass::Initial => "lightblue",
            StateClass::Active => "lightgreen",
            StateClass::Locked => "yellow",
            StateClass::Completed => "green",
            StateClass::Failed => "red",
            StateClass::Unknown => "gray",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            StateClass::Initial => "Initial",
            StateClass::Active => "Active",
            StateClass::Locked => "Locked",
            StateClass::Completed => "Completed",
            StateClass::Failed => "Failed",
            StateClass::Unknown => "Unknown",
        }
    }
}

impl State {
    pub fn new(
        utxo_ref: UtxoRef,
        datum: Option<Datum>,
        output: TxOutput,
        block: u64,
        slot: u64,
        tx_hash: String,
    ) -> Self {
        let id = utxo_ref.to_string();
        Self {
            id,
            utxo_ref,
            datum,
            output,
            block,
            slot,
            tx_hash,
            metadata: StateMetadata {
                classification: StateClass::Unknown,
                label: None,
                color: None,
            },
        }
    }

    pub fn with_class(mut self, class: StateClass) -> Self {
        self.metadata.classification = class;
        self.metadata.color = Some(class.color().to_string());
        self
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.metadata.label = Some(label);
        self
    }

    /// Get total ADA value in lovelace
    pub fn ada_value(&self) -> u64 {
        self.output
            .amount
            .iter()
            .filter(|a| a.unit == "lovelace")
            .filter_map(|a| a.quantity.parse::<u64>().ok())
            .sum()
    }

    /// Get a short display string
    pub fn display_short(&self) -> String {
        let ada = self.ada_value() as f64 / 1_000_000.0;
        let label = self
            .metadata
            .label
            .as_ref()
            .map(|l| format!(" ({})", l))
            .unwrap_or_default();
        format!("{}: {:.2} ADA{}", &self.id, ada, label)
    }
}
