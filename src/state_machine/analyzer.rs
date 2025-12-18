//! Contract pattern analyzer
//!
//! Analyzes the state machine graph to detect common smart contract patterns
//! such as linear vesting, branching marketplaces, or cyclic AMMs.

use super::StateGraph;
use petgraph::Direction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractPattern {
    /// A -> B -> C -> D
    Linear,

    /// A -> B
    ///   -> C
    Tree,

    /// A -> B -> A
    Cyclic,

    /// Mixed or Unrecognized
    Unknown,
}

impl ContractPattern {
    pub fn display_name(&self) -> &'static str {
        match self {
            ContractPattern::Linear => "Linear",
            ContractPattern::Tree => "Branching",
            ContractPattern::Cyclic => "Cyclic",
            ContractPattern::Unknown => "Complex/Unknown",
        }
    }
}

/// Analysis report containing pattern and metrics
#[derive(Debug, Clone)]
pub struct AnalysisReport {
    pub pattern: ContractPattern,
    pub branching_factor: f64,
    pub max_depth: usize,
    pub has_cycles: bool,
}

/// Detect the pattern of a state graph
pub fn detect_pattern(graph: &StateGraph) -> AnalysisReport {
    let node_count = graph.graph.node_count();

    if node_count == 0 {
        return AnalysisReport {
            pattern: ContractPattern::Unknown,
            branching_factor: 0.0,
            max_depth: 0,
            has_cycles: false,
        };
    }

    // Detect cycles
    let has_cycles = petgraph::algo::is_cyclic_directed(&graph.graph);

    // Calculate average branching factor (out-degree)
    let total_out_degree: usize = graph
        .graph
        .node_indices()
        .map(|idx| graph.graph.edges_directed(idx, Direction::Outgoing).count())
        .sum();

    let branching_factor = if node_count > 0 {
        total_out_degree as f64 / node_count as f64
    } else {
        0.0
    };

    // Classify
    let pattern = if has_cycles {
        ContractPattern::Cyclic
    } else if branching_factor <= 1.1 {
        // Allow slightly > 1 for minor deviations, but generally linear means 1 parent -> 1 child
        // strictly linear is out_degree <= 1 for all nodes
        let max_out = graph
            .graph
            .node_indices()
            .map(|idx| graph.graph.edges_directed(idx, Direction::Outgoing).count())
            .max()
            .unwrap_or(0);

        if max_out <= 1 {
            ContractPattern::Linear
        } else {
            ContractPattern::Tree
        }
    } else {
        ContractPattern::Tree
    };

    AnalysisReport {
        pattern,
        branching_factor,
        max_depth: 0, // TODO: Calculate depth if needed
        has_cycles,
    }
}
