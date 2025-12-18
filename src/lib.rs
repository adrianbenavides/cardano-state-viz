//! Cardano State Machine Visualizer
//!
//! A Terminal UI tool for visualizing Cardano eUTXO state machine transitions.
//!
//! This library provides functionality for:
//! - Fetching transaction data from multiple sources (Blockfrost, Cardano node, mock data)
//! - Parsing CBOR-encoded datums and redeemers
//! - Building state transition graphs from eUTXO chains
//! - Visualizing state machines in an interactive TUI
//! - Supporting user-defined schemas for contract-specific visualization
//! ```

pub mod cli;
pub mod config;
pub mod data_source;
pub mod error;
pub mod parser;
pub mod state_machine;

pub use config::Config;
pub use error::{Error, Result};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Initialize logging with the given log level
pub fn init_logging(level: &str) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
        assert_eq!(NAME, "cardano-state-viz");
    }
}
