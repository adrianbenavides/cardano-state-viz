//! Mock data source for testing and development
//!
//! Provides sample vesting contract transactions for testing and development.
//! The mock vesting contract locks tokens that can be unlocked over time.

use super::Transaction;
use crate::parser::PlutusData;

/// Mock script address for the vesting contract
pub const MOCK_SCRIPT_ADDRESS: &str = "addr_test1wpvesting_contract_mock_address_12345";

/// Mock data source providing hardcoded sample transactions
pub struct MockDataSource {
    transactions: Vec<Transaction>,
    script_address: String,
}

impl MockDataSource {
    pub fn new() -> Self {
        Self {
            transactions: vec![],
            script_address: MOCK_SCRIPT_ADDRESS.to_string(),
        }
    }
}

// TODO: implement
