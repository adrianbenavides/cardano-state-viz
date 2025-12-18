use crate::data_source::{Transaction, UtxoRef};
use crate::parser::schema::SchemaParser;
use crate::state_machine::{State, StateClass, StateId, Transition};
use crate::Result;
use petgraph::prelude::EdgeRef;
use petgraph::stable_graph::{EdgeIndex, NodeIndex, StableGraph};
use petgraph::Direction;
use std::collections::HashMap;

/// A directed graph representing the evolution of UTXO states for a specific Cardano script.
///
/// The `StateGraph` captures how UTXOs are transformed into new UTXOs through transactions.
/// It provides a high-level view of a smart contract's state machine by linking inputs and
/// outputs across the blockchain's history.
pub struct StateGraph {
    /// The underlying graph structure.
    /// Nodes represent UTXO states, and edges represent the transactions that move between them.
    pub graph: StableGraph<State, Transition>,

    /// A lookup table mapping unique UTXO identifiers to their internal graph indices.
    ///
    /// This index is critical for:
    /// - **Performance**: Enables O(1) lookup of graph nodes by their blockchain identifier (tx_hash#index).
    /// - **Graph Construction**: Allows efficient linking of transitions by quickly finding the
    ///   source and target nodes for a transaction's inputs and outputs.
    /// - **Data Integrity**: Ensures each unique UTXO is represented by exactly one node in the graph.
    pub state_index: HashMap<StateId, NodeIndex>,

    /// The Bech32 address of the script whose states are being tracked.
    pub script_address: String,
}

impl StateGraph {
    pub fn new(script_address: String) -> Self {
        Self {
            graph: StableGraph::new(),
            state_index: HashMap::new(),
            script_address,
        }
    }

    /// Adds a state to the graph and updates the state index.
    pub fn add_state(&mut self, state: State) -> NodeIndex {
        let node_index = self.graph.add_node(state.clone());
        self.state_index.insert(state.id, node_index);
        node_index
    }

    /// Adds a transition to the graph, linking two existing states.
    pub fn add_transition(&mut self, transition: Transition) -> Option<EdgeIndex> {
        if let (Some(&from_idx), Some(&to_idx)) = (
            self.state_index.get(&transition.from_state),
            self.state_index.get(&transition.to_state),
        ) {
            Some(self.graph.add_edge(from_idx, to_idx, transition))
        } else {
            None
        }
    }

    pub fn build_from_transactions(
        transactions: &[Transaction],
        script_address: &str,
        parser: Option<&SchemaParser>,
    ) -> Result<Self> {
        let mut graph = Self::new(script_address.to_string());

        // Collect all outputs at the script address
        let mut all_outputs: Vec<(String, usize, &Transaction)> = Vec::new();
        for tx in transactions {
            for (output_idx, output) in tx.outputs.iter().enumerate() {
                if output.address == script_address {
                    all_outputs.push((tx.hash.clone(), output_idx, tx));
                }
            }
        }

        // Create state nodes for each output
        for (tx_hash, output_idx, tx) in &all_outputs {
            let utxo_ref = UtxoRef::new(tx_hash, *output_idx as u64);
            let output = &tx.outputs[*output_idx];
            graph.add_state(State::new(
                utxo_ref,
                output.datum.clone(),
                output.clone(),
                tx.block,
                tx.slot,
                tx.hash.clone(),
            ));
        }

        // Build transitions by linking inputs to outputs
        // For each transaction:
        //  - Identify inputs that consume UTXOs at the script address
        //  - Identify outputs that create UTXOs at the script address
        //  - Create transitions between consumed and created states
        for tx in transactions {
            // Find inputs that consume UTXOs
            let consumed_states: Vec<StateId> = tx
                .inputs
                .iter()
                .filter_map(|input| {
                    let state_id = input.utxo_ref.to_string();
                    if graph.state_index.contains_key(&state_id) {
                        Some(state_id)
                    } else {
                        None
                    }
                })
                .collect();

            // Find outputs that create UTXOs
            let created_states: Vec<StateId> = tx
                .outputs
                .iter()
                .enumerate()
                .filter_map(|(idx, output)| {
                    // Only creates states for outputs sent to THIS script address
                    if output.address == script_address {
                        let utxo_ref = UtxoRef::new(&tx.hash, idx as u64);
                        Some(utxo_ref.to_string())
                    } else {
                        None
                    }
                })
                .collect();

            // Create transitions: each consumed state -> each created state
            for from_state in &consumed_states {
                for to_state in &created_states {
                    // Find redeemer for this input if available
                    let redeemer = tx
                        .witnesses
                        .redeemers
                        .iter()
                        .find(|r| {
                            // Match redeemer by index
                            // TODO: would need to properly match spend redeemers to inputs
                            r.index < tx.inputs.len() as u64
                        })
                        .cloned();

                    let mut transition = Transition::new(
                        from_state.clone(),
                        to_state.clone(),
                        tx.hash.clone(),
                        redeemer.clone(),
                    );

                    // Apply schema-based label if available
                    if let Some(parser) = parser
                        && let Some(r) = &redeemer
                        && let Some(parsed) = &r.parsed
                        && let Some(name) = &parsed.name
                        && let Some(label) = parser.label_transition(name)
                    {
                        transition = transition.with_label(label);
                    }

                    graph.add_transition(transition);
                }
            }
        }

        graph.classify_states(parser);
        Ok(graph)
    }

    /// Classify states based on their position in the graph and schema rules
    fn classify_states(&mut self, parser: Option<&SchemaParser>) {
        // Collect node indices and their classifications first
        let classifications: Vec<(NodeIndex, StateClass)> = self
            .graph
            .node_indices()
            .map(|node_idx| {
                // Default topological classification
                let has_incoming = self
                    .graph
                    .edges_directed(node_idx, Direction::Incoming)
                    .count()
                    > 0;
                let has_outgoing = self
                    .graph
                    .edges_directed(node_idx, Direction::Outgoing)
                    .count()
                    > 0;
                let mut classification = match (has_incoming, has_outgoing) {
                    (false, true) => StateClass::Initial,  // No inputs, has outputs
                    (false, false) => StateClass::Initial, // No inputs, no outputs (initial isolated)
                    (true, false) => StateClass::Completed, // Has inputs, no outputs (terminal)
                    (true, true) => StateClass::Active,    // Has both (active transition)
                };

                // Override with schema-based classification if available
                if let Some(parser) = parser
                    && let Some(state) = self.graph.node_weight(node_idx)
                    && let Some(schema_class) = parser.classify_state(state)
                {
                    classification = schema_class;
                }
                (node_idx, classification)
            })
            .collect();

        // Apply classifications to states
        for (node_idx, classification) in classifications {
            if let Some(state) = self.graph.node_weight_mut(node_idx) {
                state.metadata.classification = classification;
                state.metadata.color = Some(classification.color().to_string());
            }
        }
    }

    /// Find all initial states (no incoming edges)
    pub fn find_initial_states(&self) -> Vec<&State> {
        self.graph
            .node_indices()
            .filter(|&idx| self.graph.edges_directed(idx, Direction::Incoming).count() == 0)
            .filter_map(|idx| self.graph.node_weight(idx))
            .collect()
    }

    /// Find all terminal states (no outgoing edges)
    pub fn find_terminal_states(&self) -> Vec<&State> {
        self.graph
            .node_indices()
            .filter(|&idx| self.graph.edges_directed(idx, Direction::Outgoing).count() == 0)
            .filter_map(|idx| self.graph.node_weight(idx))
            .collect()
    }

    /// Export to DOT format for Graphviz
    pub fn to_dot(&self) -> String {
        let mut dot = "digraph StateGraph {\n".to_string();
        dot.push_str("  rankdir=LR;\n");
        dot.push_str("  node [shape=box, style=filled];\n\n");

        // Add nodes
        for (state_id, &node_idx) in &self.state_index {
            if let Some(state) = self.graph.node_weight(node_idx) {
                let color = state.metadata.color.as_deref().unwrap_or("gray");
                let label = state.display_short();
                let safe_id = state_id.replace(['#', '-'], "_");

                dot.push_str(&format!(
                    "  \"{}\" [label=\"{}\", fillcolor=\"{}\"];\n",
                    safe_id, label, color
                ));
            }
        }

        dot.push('\n');

        // Add edges
        for edge_idx in self.graph.edge_indices() {
            if let Some((from_idx, to_idx)) = self.graph.edge_endpoints(edge_idx)
                && let (Some(from_state), Some(to_state), Some(transition)) = (
                    self.graph.node_weight(from_idx),
                    self.graph.node_weight(to_idx),
                    self.graph.edge_weight(edge_idx),
                )
            {
                let from_id = from_state.id.replace(['#', '-'], "_");
                let to_id = to_state.id.replace(['#', '-'], "_");
                let label = transition.display_label();

                dot.push_str(&format!(
                    "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                    from_id, to_id, label
                ));
            }
        }

        dot.push_str("}\n");

        // Export to {date}.graph.dot file
        let filename = format!("{}.graph.dot", chrono::Utc::now().format("%Y%m%d%H%M%S"));
        if let Ok(mut file) = std::fs::File::create(&filename) {
            use std::io::Write;
            let _ = file.write_all(dot.as_bytes());
            format!("Graph exported to {}", filename)
        } else {
            "Failed to export graph to file".to_string()
        }
    }

    /// Get a state by its ID
    pub fn get_state(&self, state_id: &StateId) -> Option<&State> {
        self.state_index
            .get(state_id)
            .and_then(|&idx| self.graph.node_weight(idx))
    }

    /// Get all states
    pub fn all_states(&self) -> Vec<&State> {
        self.graph
            .node_indices()
            .filter_map(|idx| self.graph.node_weight(idx))
            .collect()
    }

    /// Get outgoing transitions from a state
    pub fn outgoing_transitions(&self, state_id: &StateId) -> Vec<&Transition> {
        if let Some(&node_idx) = self.state_index.get(state_id) {
            self.graph
                .edges_directed(node_idx, Direction::Outgoing)
                .filter_map(|edge| self.graph.edge_weight(edge.id()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get incoming transitions to a state
    pub fn incoming_transitions(&self, state_id: &StateId) -> Vec<&Transition> {
        if let Some(&node_idx) = self.state_index.get(state_id) {
            self.graph
                .edges_directed(node_idx, Direction::Incoming)
                .filter_map(|edge| self.graph.edge_weight(edge.id()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get graph statistics
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            total_states: self.graph.node_count(),
            total_transitions: self.graph.edge_count(),
            initial_states: self.find_initial_states().len(),
            terminal_states: self.find_terminal_states().len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphStats {
    pub total_states: usize,
    pub total_transitions: usize,
    pub initial_states: usize,
    pub terminal_states: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::{Asset, Datum, TxInput, TxOutput, UtxoRef, Witnesses};

    fn create_test_transaction(
        hash: &str,
        block: u64,
        slot: u64,
        inputs: Vec<TxInput>,
        outputs: Vec<TxOutput>,
    ) -> Transaction {
        Transaction {
            hash: hash.to_string(),
            block,
            slot,
            inputs,
            outputs,
            witnesses: Witnesses::default(),
            metadata: None,
        }
    }

    #[test]
    fn test_empty_graph() {
        let graph = StateGraph::new("addr_script".to_string());
        assert_eq!(graph.graph.node_count(), 0);
        assert_eq!(graph.graph.edge_count(), 0);
    }

    #[test]
    fn test_build_linear_graph() {
        // Create a simple linear flow: tx1 creates state1, tx2 consumes state1 and creates state2
        let script_addr = "addr_script";

        let tx1 = create_test_transaction(
            "tx1",
            100,
            1000,
            vec![],
            vec![TxOutput {
                address: script_addr.to_string(),
                amount: vec![Asset::lovelace(10_000_000)],
                datum: Some(Datum {
                    hash: "datum1".to_string(),
                    raw_cbor: vec![0x01],
                    parsed: None,
                }),
                script_ref: None,
            }],
        );

        let tx2 = create_test_transaction(
            "tx2",
            200,
            2000,
            vec![TxInput {
                utxo_ref: UtxoRef::new("tx1", 0),
                address: Some(script_addr.to_string()),
                amount: Some(vec![Asset::lovelace(10_000_000)]),
            }],
            vec![TxOutput {
                address: script_addr.to_string(),
                amount: vec![Asset::lovelace(9_000_000)],
                datum: Some(Datum {
                    hash: "datum2".to_string(),
                    raw_cbor: vec![0x02],
                    parsed: None,
                }),
                script_ref: None,
            }],
        );

        let transactions = vec![tx1, tx2];
        let graph = StateGraph::build_from_transactions(&transactions, script_addr, None).unwrap();

        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 1);

        let initial = graph.find_initial_states();
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].id, "tx1#0");

        let terminal = graph.find_terminal_states();
        assert_eq!(terminal.len(), 1);
        assert_eq!(terminal[0].id, "tx2#0");
    }

    #[test]
    fn test_state_classification() {
        let script_addr = "addr_script";

        let tx1 = create_test_transaction(
            "tx1",
            100,
            1000,
            vec![],
            vec![TxOutput {
                address: script_addr.to_string(),
                amount: vec![Asset::lovelace(10_000_000)],
                datum: None,
                script_ref: None,
            }],
        );

        let transactions = vec![tx1];
        let graph = StateGraph::build_from_transactions(&transactions, script_addr, None).unwrap();

        let state = graph.get_state(&"tx1#0".to_string()).unwrap();
        assert_eq!(state.metadata.classification, StateClass::Initial);
    }

    #[test]
    fn test_to_dot_output() {
        let script_addr = "addr_script";

        let tx1 = create_test_transaction(
            "tx1",
            100,
            1000,
            vec![],
            vec![TxOutput {
                address: script_addr.to_string(),
                amount: vec![Asset::lovelace(10_000_000)],
                datum: None,
                script_ref: None,
            }],
        );

        let transactions = vec![tx1];
        let graph = StateGraph::build_from_transactions(&transactions, script_addr, None).unwrap();

        let dot = graph.to_dot();
        assert!(dot.contains("digraph StateGraph"));
        assert!(dot.contains("tx1_0"));
        assert!(dot.contains("lightblue")); // Initial state color
    }

    #[test]
    fn test_graph_stats() {
        let script_addr = "addr_script";

        let tx1 = create_test_transaction(
            "tx1",
            100,
            1000,
            vec![],
            vec![TxOutput {
                address: script_addr.to_string(),
                amount: vec![Asset::lovelace(10_000_000)],
                datum: None,
                script_ref: None,
            }],
        );

        let tx2 = create_test_transaction(
            "tx2",
            200,
            2000,
            vec![TxInput {
                utxo_ref: UtxoRef::new("tx1", 0),
                address: Some(script_addr.to_string()),
                amount: Some(vec![Asset::lovelace(10_000_000)]),
            }],
            vec![TxOutput {
                address: script_addr.to_string(),
                amount: vec![Asset::lovelace(9_000_000)],
                datum: None,
                script_ref: None,
            }],
        );

        let transactions = vec![tx1, tx2];
        let graph = StateGraph::build_from_transactions(&transactions, script_addr, None).unwrap();

        let stats = graph.stats();
        assert_eq!(stats.total_states, 2);
        assert_eq!(stats.total_transitions, 1);
        assert_eq!(stats.initial_states, 1);
        assert_eq!(stats.terminal_states, 1);
    }
}
