//! State machine module - Build and analyze state transition graphs

use crate::{Result, data_source::Transaction};

pub mod analyzer;
pub mod graph;
pub mod state;
pub mod transition;

// Re-export key types
pub use graph::{GraphStats, StateGraph};
pub use state::{State, StateClass, StateId, StateMetadata};
pub use transition::Transition;

/// Build state graph from transactions
pub fn build_state_graph(
    transactions: &[Transaction],
    script_address: &str,
    parser: Option<&crate::parser::schema::SchemaParser>,
) -> Result<StateGraph> {
    StateGraph::build_from_transactions(transactions, script_address, parser)
}
