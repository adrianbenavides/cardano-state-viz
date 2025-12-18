//! Transition representation

use crate::data_source::Redeemer;
use crate::state_machine::StateId;
use serde::{Deserialize, Serialize};

/// A transition between states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from_state: StateId,
    pub to_state: StateId,
    pub tx_hash: String,
    pub redeemer: Option<Redeemer>,
    pub label: Option<String>,
}

impl Transition {
    pub fn new(
        from_state: StateId,
        to_state: StateId,
        tx_hash: String,
        redeemer: Option<Redeemer>,
    ) -> Self {
        Self {
            from_state,
            to_state,
            tx_hash,
            redeemer,
            label: None,
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    /// Get display label for the transition
    pub fn display_label(&self) -> String {
        if let Some(label) = &self.label {
            label.clone()
        } else if let Some(redeemer) = &self.redeemer {
            format!("{:?}", redeemer.tag)
        } else {
            "transition".to_string()
        }
    }
}
