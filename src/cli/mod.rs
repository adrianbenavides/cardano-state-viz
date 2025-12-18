//! CLI module
//!
//! This module defines the command-line interface using clap and implements
//! the command execution logic.

use crate::{Config, Result};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::time::Duration;

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

        /// Disable caching
        #[arg(long)]
        no_cache: bool,

        /// Cache TTL
        #[arg(long, default_value = "1d", value_parser = duration_parser)]
        cache_ttl: Duration,

        /// Limit the number of transactions to fetch
        #[arg(long)]
        max_transactions: Option<usize>,
    },

    /// Watch for new transactions in real-time
    Watch {
        /// Script address to watch
        #[arg(short, long)]
        address: String,

        /// Data source type
        #[arg(short, long, value_enum, default_value = "blockfrost")]
        source: DataSourceType,

        /// Polling interval
        #[arg(long, default_value = "30", value_parser = duration_parser)]
        interval: Duration,

        /// Path to schema file
        #[arg(long)]
        schema: Option<PathBuf>,

        /// Disable caching
        #[arg(long)]
        no_cache: bool,

        /// Cache TTL
        #[arg(long, default_value = "3600", value_parser = duration_parser)]
        cache_ttl: Duration,

        /// Limit the number of transactions to fetch
        #[arg(long)]
        max_transactions: Option<usize>,
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

fn duration_parser(arg: &str) -> std::result::Result<Duration, clap::Error> {
    parse_duration(arg).map_err(|_| clap::Error::raw(ErrorKind::InvalidValue, "Invalid duration"))
}

/// It accepts a number and a unit:
///
///  - h: hour
///  - m: minute
///  - s: second
///  - ms: millisecond
///
/// For example: 1ms, 2s, 10m
/// Defaults to seconds if no unit is provided.
fn parse_duration(arg: &str) -> std::result::Result<Duration, clap::Error> {
    let needles = regex::Regex::new(r"(?P<numeric_duration>[0-9]+)(?P<length_sigil>d|h|m|s|ms)?$")
        .unwrap()
        .captures(arg)
        .ok_or_else(|| clap::Error::raw(ErrorKind::InvalidValue, "Invalid duration"))?;
    let time = needles["numeric_duration"]
        .parse::<u64>()
        .map_err(|_| clap::Error::raw(ErrorKind::InvalidValue, "Invalid duration"))?;

    match needles.name("length_sigil") {
        Some(n) => match n.as_str() {
            "ms" => Ok(Duration::from_millis(time)),
            "s" => Ok(Duration::from_secs(time)),
            "m" => Ok(Duration::from_secs(60 * time)),
            "h" => Ok(Duration::from_secs(60 * 60 * time)),
            "d" => Ok(Duration::from_secs(60 * 60 * 24 * time)),
            _ => unreachable!("Alternatives excluded by regex."),
        },
        None => Ok(Duration::from_secs(time)),
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
