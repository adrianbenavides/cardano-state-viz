//! TUI application state

use crate::data_source::Transaction;
use crate::state_machine::{
    State, StateGraph, StateId,
    analyzer::{AnalysisReport, detect_pattern},
};
use ratatui::widgets::ListState;

/// TUI application state
pub struct App {
    pub state_graph: StateGraph,
    pub analysis_report: AnalysisReport,
    pub selected_state_index: usize,
    pub state_list_state: ListState,
    pub selected_transaction_index: usize,
    pub transaction_list_state: ListState,
    pub view_mode: ViewMode,
    pub should_quit: bool,
    pub show_hex_view: bool, // For datum inspector: hex vs decoded view
    states_list: Vec<StateId>,
    transactions: Vec<Transaction>,
    view_stack: Vec<ViewMode>,
}

/// View modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    GraphOverview,
    StateDetail,
    TransactionList,
    DatumInspector,
    PatternAnalysis,
    Help,
}

impl App {
    pub fn new(state_graph: StateGraph, transactions: Vec<Transaction>) -> Self {
        // Run pattern analysis
        let analysis_report = detect_pattern(&state_graph);

        // Get all states and sort by block/slot for consistent ordering
        let mut states: Vec<&State> = state_graph.all_states();
        states.sort_by_key(|s| (s.block, s.slot));
        let states_list: Vec<StateId> = states.iter().map(|s| s.id.clone()).collect();

        let mut state_list_state = ListState::default();
        if !states_list.is_empty() {
            state_list_state.select(Some(0));
        }

        let mut transaction_list_state = ListState::default();
        if !transactions.is_empty() {
            transaction_list_state.select(Some(0));
        }

        Self {
            state_graph,
            analysis_report,
            selected_state_index: 0,
            state_list_state,
            selected_transaction_index: 0,
            transaction_list_state,
            view_mode: ViewMode::GraphOverview,
            should_quit: false,
            show_hex_view: false,
            states_list,
            transactions,
            view_stack: Vec::new(),
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if self.view_mode != mode {
            self.view_stack.push(self.view_mode);
            self.view_mode = mode;
        }
    }

    pub fn pop_view_mode(&mut self) {
        if let Some(prev_mode) = self.view_stack.pop() {
            self.view_mode = prev_mode;
        } else {
            // If stack is empty, default to GraphOverview
            if self.view_mode != ViewMode::GraphOverview {
                self.view_mode = ViewMode::GraphOverview;
            }
        }
    }

    pub fn select_next(&mut self) {
        if !self.states_list.is_empty() {
            self.selected_state_index = (self.selected_state_index + 1) % self.states_list.len();
            self.state_list_state
                .select(Some(self.selected_state_index));
        }
    }

    pub fn select_previous(&mut self) {
        if !self.states_list.is_empty() {
            if self.selected_state_index == 0 {
                self.selected_state_index = self.states_list.len() - 1;
            } else {
                self.selected_state_index -= 1;
            }
            self.state_list_state
                .select(Some(self.selected_state_index));
        }
    }

    pub fn get_selected_state(&self) -> Option<&State> {
        self.states_list
            .get(self.selected_state_index)
            .and_then(|id| self.state_graph.get_state(id))
    }

    pub fn states_list(&self) -> &[StateId] {
        &self.states_list
    }

    pub fn select_next_transaction(&mut self) {
        if !self.transactions.is_empty() {
            self.selected_transaction_index =
                (self.selected_transaction_index + 1) % self.transactions.len();
            self.transaction_list_state
                .select(Some(self.selected_transaction_index));
        }
    }

    pub fn select_previous_transaction(&mut self) {
        if !self.transactions.is_empty() {
            if self.selected_transaction_index == 0 {
                self.selected_transaction_index = self.transactions.len() - 1;
            } else {
                self.selected_transaction_index -= 1;
            }
            self.transaction_list_state
                .select(Some(self.selected_transaction_index));
        }
    }

    pub fn get_selected_transaction(&self) -> Option<&Transaction> {
        self.transactions.get(self.selected_transaction_index)
    }

    pub fn transactions(&self) -> &[Transaction] {
        &self.transactions
    }

    pub fn toggle_hex_view(&mut self) {
        self.show_hex_view = !self.show_hex_view;
    }

    pub fn update_data(&mut self, state_graph: StateGraph, transactions: Vec<Transaction>) {
        // Run pattern analysis
        self.analysis_report = detect_pattern(&state_graph);

        // Preserve selected state ID if possible
        let selected_id = self.states_list.get(self.selected_state_index).cloned();

        self.state_graph = state_graph;
        self.transactions = transactions;

        // Rebuild states list
        let mut states: Vec<&State> = self.state_graph.all_states();
        states.sort_by_key(|s| (s.block, s.slot));
        self.states_list = states.iter().map(|s| s.id.clone()).collect();

        // Restore selection or reset
        if let Some(id) = selected_id {
            if let Some(pos) = self.states_list.iter().position(|x| x == &id) {
                self.selected_state_index = pos;
                self.state_list_state.select(Some(pos));
            } else {
                self.selected_state_index = 0;
                self.state_list_state
                    .select(if !self.states_list.is_empty() {
                        Some(0)
                    } else {
                        None
                    });
            }
        } else {
            self.selected_state_index = 0;
            self.state_list_state
                .select(if !self.states_list.is_empty() {
                    Some(0)
                } else {
                    None
                });
        }

        // Reset transaction selection (simplification)
        self.selected_transaction_index = 0;
        self.transaction_list_state
            .select(if !self.transactions.is_empty() {
                Some(0)
            } else {
                None
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::{Asset, Datum, Transaction, TxInput, TxOutput, UtxoRef, Witnesses};
    use crate::state_machine::StateGraph;

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

    fn create_test_data() -> (StateGraph, Vec<Transaction>) {
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

        let tx3 = create_test_transaction(
            "tx3",
            150,
            1500,
            vec![],
            vec![TxOutput {
                address: script_addr.to_string(),
                amount: vec![Asset::lovelace(5_000_000)],
                datum: None,
                script_ref: None,
            }],
        );

        let transactions = vec![tx1, tx2, tx3];
        let graph = StateGraph::build_from_transactions(&transactions, script_addr, None).unwrap();
        (graph, transactions)
    }

    #[test]
    fn test_app_creation() {
        let (graph, transactions) = create_test_data();
        let app = App::new(graph, transactions);

        assert_eq!(app.selected_state_index, 0);
        assert_eq!(app.view_mode, ViewMode::GraphOverview);
        assert!(!app.should_quit);
        assert_eq!(app.states_list.len(), 3);
    }

    #[test]
    fn test_app_navigation() {
        let (graph, transactions) = create_test_data();
        let mut app = App::new(graph, transactions);

        // Initially at index 0
        assert_eq!(app.selected_state_index, 0);

        // Next wraps around
        app.select_next();
        assert_eq!(app.selected_state_index, 1);
        app.select_next();
        assert_eq!(app.selected_state_index, 2);
        app.select_next();
        assert_eq!(app.selected_state_index, 0); // Wrapped

        // Previous wraps backward
        app.select_previous();
        assert_eq!(app.selected_state_index, 2); // Wrapped backward
        app.select_previous();
        assert_eq!(app.selected_state_index, 1);
    }

    #[test]
    fn test_app_view_modes() {
        let (graph, transactions) = create_test_data();
        let mut app = App::new(graph, transactions);

        assert_eq!(app.view_mode, ViewMode::GraphOverview);

        app.set_view_mode(ViewMode::StateDetail);
        assert_eq!(app.view_mode, ViewMode::StateDetail);

        app.set_view_mode(ViewMode::TransactionList);
        assert_eq!(app.view_mode, ViewMode::TransactionList);

        app.set_view_mode(ViewMode::Help);
        assert_eq!(app.view_mode, ViewMode::Help);

        app.set_view_mode(ViewMode::GraphOverview);
        assert_eq!(app.view_mode, ViewMode::GraphOverview);
    }

    #[test]
    fn test_app_get_selected_state() {
        let (graph, transactions) = create_test_data();
        let mut app = App::new(graph, transactions);

        // First state should be selected
        let state = app.get_selected_state();
        assert!(state.is_some());
        let first_id = state.unwrap().id.clone();

        // Navigate and check
        app.select_next();
        let state2 = app.get_selected_state();
        assert!(state2.is_some());
        assert_ne!(first_id, state2.unwrap().id);
    }

    #[test]
    fn test_app_quit() {
        let (graph, transactions) = create_test_data();
        let mut app = App::new(graph, transactions);

        assert!(!app.should_quit);
        app.quit();
        assert!(app.should_quit);
    }

    #[test]
    fn test_app_states_sorted_by_block_slot() {
        let (graph, transactions) = create_test_data();
        let app = App::new(graph, transactions);

        let states = app.states_list();
        assert_eq!(states.len(), 3);

        // Should be sorted by (block, slot)
        // tx1: block 100, slot 1000
        // tx3: block 150, slot 1500
        // tx2: block 200, slot 2000
        assert_eq!(states[0], "tx1#0");
        assert_eq!(states[1], "tx3#0");
        assert_eq!(states[2], "tx2#0");
    }

    #[test]
    fn test_app_navigation_empty_graph() {
        let empty_graph = StateGraph::new("addr_test".to_string());
        let mut app = App::new(empty_graph, vec![]);

        // Should handle empty state gracefully
        assert_eq!(app.states_list.len(), 0);
        app.select_next(); // Should not panic
        app.select_previous(); // Should not panic
        assert!(app.get_selected_state().is_none());
    }

    #[test]
    fn test_app_transaction_navigation() {
        let (graph, transactions) = create_test_data();
        let mut app = App::new(graph, transactions);

        // Initially at index 0
        assert_eq!(app.selected_transaction_index, 0);

        // Next wraps around
        app.select_next_transaction();
        assert_eq!(app.selected_transaction_index, 1);
        app.select_next_transaction();
        assert_eq!(app.selected_transaction_index, 2);
        app.select_next_transaction();
        assert_eq!(app.selected_transaction_index, 0); // Wrapped

        // Previous wraps backward
        app.select_previous_transaction();
        assert_eq!(app.selected_transaction_index, 2); // Wrapped backward
    }
}
