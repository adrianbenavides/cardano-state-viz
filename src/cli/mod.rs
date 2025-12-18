//! CLI module
//!
//! This module defines the command-line interface using clap and implements
//! the command execution logic.

use crate::{Config, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

pub mod commands;
pub mod output;

/// Cardano State Machine Visualizer CLI
#[derive(Parser, Debug)]
#[command(name = "cardano-state-viz")]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Analyze transactions at a script address
    Analyze {
        /// Script address to analyze
        #[arg(short, long)]
        address: String,

        /// Data source type
        #[arg(short, long, value_enum, default_value = "mock")]
        source: DataSourceType,

        /// Network (mainnet, preprod, preview)
        #[arg(short, long)]
        network: Option<String>,

        /// Blockfrost API key (overrides config)
        #[arg(long, env = "BLOCKFROST_API_KEY")]
        api_key: Option<String>,

        /// Path to schema file
        #[arg(long)]
        schema: Option<PathBuf>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "tui")]
        output: OutputFormat,

        /// Enable caching
        #[arg(long)]
        cache: bool,

        /// Cache TTL in seconds
        #[arg(long, default_value = "3600")]
        cache_ttl: u64,
    },

    /// Watch for new transactions in real-time
    Watch {
        /// Script address to watch
        #[arg(short, long)]
        address: String,

        /// Data source type
        #[arg(short, long, value_enum, default_value = "blockfrost")]
        source: DataSourceType,

        /// Polling interval in seconds
        #[arg(long, default_value = "30")]
        interval: u64,

        /// Path to schema file
        #[arg(long)]
        schema: Option<PathBuf>,
    },

    /// Validate a schema file
    SchemaValidate {
        /// Path to schema file
        schema: PathBuf,
    },
}

/// Data source types
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DataSourceType {
    /// Mock data for testing
    Mock,
    /// Blockfrost API
    Blockfrost,
    /// Local Cardano node
    Node,
}

/// Output format types
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// JSON output
    Json,
    /// DOT format (Graphviz)
    Dot,
    /// Interactive TUI
    Tui,
    /// Plain text table
    Table,
}

/// Execute the CLI command
pub async fn execute(args: Cli, config: Config) -> Result<()> {
    match args.command {
        Commands::Analyze { .. } => commands::analyze::execute(args, config).await,
        Commands::Watch { .. } => commands::watch::execute(args, config).await,
        Commands::SchemaValidate { schema } => commands::schema_validate::execute(schema),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test basic parsing
        let cli = Cli::try_parse_from([
            "cardano-state-viz",
            "analyze",
            "--address",
            "addr_test1...",
            "--source",
            "mock",
        ]);

        assert!(cli.is_ok());
    }
}
