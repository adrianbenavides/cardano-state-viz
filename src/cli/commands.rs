//! CLI command implementations
//!
//! This module contains the implementation for each CLI command.

use crate::data_source::Transaction;
use crate::parser::schema::SchemaParser;
use crate::{Config, Result, cli::Cli};
use std::path::PathBuf;

/// Process transactions: hydrate datums from witnesses and parse datums/redeemers
fn process_transactions(transactions: &mut [Transaction], schema_parser: Option<&SchemaParser>) {
    for tx in transactions {
        // Clone witnesses to avoid borrow checker issues when iterating outputs
        let witnesses = tx.witnesses.clone();

        for output in &mut tx.outputs {
            if let Some(datum) = &mut output.datum {
                // Hydrate if missing CBOR (lookup hash in witnesses)
                if datum.raw_cbor.is_empty() {
                    // Find datum in witnesses
                    if let Some(resolved) = witnesses.datums.iter().find(|d| d.hash == datum.hash)
                        && !resolved.raw_cbor.is_empty()
                    {
                        datum.raw_cbor = resolved.raw_cbor.clone();
                    }
                }

                // Parse using selected parser
                if !datum.raw_cbor.is_empty() {
                    let parser: &dyn crate::parser::Parser = if let Some(p) = schema_parser {
                        p
                    } else {
                        &crate::parser::GenericParser
                    };

                    match parser.parse_datum(&datum.raw_cbor) {
                        Ok(parsed) => datum.parsed = Some(parsed),
                        Err(e) => tracing::debug!("Failed to parse datum: {}", e),
                    }
                }
            }
        }

        // Also parse redeemers if possible
        for redeemer in &mut tx.witnesses.redeemers {
            if !redeemer.raw_cbor.is_empty() {
                let parser: &dyn crate::parser::Parser = if let Some(p) = schema_parser {
                    p
                } else {
                    &crate::parser::GenericParser
                };

                match parser.parse_redeemer(&redeemer.raw_cbor) {
                    Ok(parsed) => redeemer.parsed = Some(parsed),
                    Err(e) => tracing::debug!("Failed to parse redeemer: {}", e),
                }
            }
        }
    }
}

/// Analyze command implementation
pub mod analyze {
    use super::*;
    use crate::parser::schema::ContractSchema;
    use crate::{
        cli::{Commands, OutputFormat},
        data_source::{QueryParams, create_data_source},
    };

    /// Execute the analyze command
    pub async fn execute(args: Cli, config: Config) -> Result<()> {
        // Extract command-specific arguments
        let (address, source, output_format, schema, cache, cache_ttl, max_transactions) =
            match args.command {
                Commands::Analyze {
                    address,
                    source,
                    output,
                    schema,
                    no_cache,
                    cache_ttl,
                    max_transactions,
                    ..
                } => (
                    address,
                    source,
                    output,
                    schema,
                    !no_cache,
                    cache_ttl,
                    max_transactions,
                ),
                _ => unreachable!("analyze::execute called with wrong command"),
            };

        tracing::info!("Analyzing address: {}", address);
        tracing::debug!("Using data source: {:?}", source);

        // Create data source based on args.source
        let data_source = create_data_source(source, &config, cache, cache_ttl).await?;

        // Fetch transactions for the given address
        tracing::info!("Fetching transactions...");
        let mut query_params = QueryParams::default();
        if let Some(limit) = max_transactions {
            query_params = query_params.limit(limit);
        }

        let mut transactions = data_source
            .get_transactions_by_address(&address, query_params)
            .await?;

        tracing::info!("Found {} transactions", transactions.len());

        // Prepare parser (Schema or Generic)
        let schema_parser = if let Some(schema_path) = schema {
            tracing::info!("Loading schema from {:?}", schema_path);
            let contract_schema = ContractSchema::from_file(schema_path)?;
            Some(SchemaParser::new(contract_schema))
        } else {
            None
        };

        // Hydrate and parse datums in transactions
        tracing::info!("Parsing datums...");
        process_transactions(&mut transactions, schema_parser.as_ref());

        // Collect all datums for list output
        let mut all_datums = Vec::new();
        for tx in &transactions {
            for (idx, output) in tx.outputs.iter().enumerate() {
                if let Some(datum) = &output.datum {
                    all_datums.push((tx.hash.clone(), idx, datum.clone()));
                }
            }
        }

        tracing::info!("Extracted {} datums", all_datums.len());

        // Output in requested format
        match output_format {
            OutputFormat::Json => {
                crate::cli::output::output_json(
                    &mut std::io::stdout(),
                    &transactions,
                    &all_datums,
                )?;
            }
            OutputFormat::Table => {
                crate::cli::output::output_table(
                    &mut std::io::stdout(),
                    &transactions,
                    &all_datums,
                )?;
            }
            OutputFormat::Dot => {
                tracing::info!("Building state graph for DOT output...");
                let graph = crate::state_machine::build_state_graph(
                    &transactions,
                    &address,
                    schema_parser.as_ref(),
                )?;
                let dot = graph.to_dot();
                println!("{}", dot);
            }
            OutputFormat::Tui => {
                tracing::info!("Launching TUI...");
                let graph = crate::state_machine::build_state_graph(
                    &transactions,
                    &address,
                    schema_parser.as_ref(),
                )?;
                crate::tui::run(graph, transactions, None)?;
            }
        }

        Ok(())
    }
}

/// Watch command implementation
pub mod watch {
    use super::*;
    use crate::parser::schema::ContractSchema;
    use crate::{
        cli::Commands,
        data_source::{QueryParams, create_data_source},
    };
    use std::time::Duration;
    use tokio::sync::mpsc;

    /// Execute the watch command
    pub async fn execute(args: Cli, config: Config) -> Result<()> {
        let (address, source, interval_secs, schema, max_transactions) = match args.command {
            Commands::Watch {
                address,
                source,
                interval,
                schema,
                max_transactions,
                ..
            } => (
                address,
                source,
                interval.as_secs(),
                schema,
                max_transactions,
            ),
            _ => unreachable!("watch::execute called with wrong command"),
        };

        // Initial fetch
        tracing::info!("Fetching initial data...");
        let data_source =
            create_data_source(source, &config, false, Duration::from_secs(0)).await?;

        let mut query_params = QueryParams::default();
        if let Some(limit) = max_transactions {
            query_params = query_params.limit(limit);
        }

        let mut transactions = data_source
            .get_transactions_by_address(&address, query_params)
            .await?;

        // Prepare parser
        let schema_parser = if let Some(schema_path) = &schema {
            tracing::info!("Loading schema from {:?}", schema_path);
            let contract_schema = ContractSchema::from_file(schema_path)?;
            Some(SchemaParser::new(contract_schema))
        } else {
            None
        };

        // Process initial data (hydrate/parse)
        process_transactions(&mut transactions, schema_parser.as_ref());

        let graph = crate::state_machine::build_state_graph(
            &transactions,
            &address,
            schema_parser.as_ref(),
        )?;

        // Setup channel
        let (tx_sender, rx_receiver) = mpsc::channel(1);

        // Spawn watcher task
        let address_clone = address.clone();
        let source_clone = source;
        let config_clone = config.clone();
        let schema_parser_clone = schema_parser
            .as_ref()
            .map(|p| SchemaParser::new(p.schema.clone()));

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            interval.tick().await; // First tick is immediate, but we already did initial load, so we skip it.
            loop {
                // Fetch new data
                // TODO: For simplicity, re-fetch all. In prod, use from_block/slot.
                if let Ok(ds) =
                    create_data_source(source_clone, &config_clone, false, Duration::from_secs(0))
                        .await
                {
                    let mut query_params = QueryParams::default();
                    if let Some(limit) = max_transactions {
                        query_params = query_params.limit(limit);
                    }

                    if let Ok(mut new_txs) = ds
                        .get_transactions_by_address(&address_clone, query_params)
                        .await
                    {
                        // Process
                        super::process_transactions(&mut new_txs, schema_parser_clone.as_ref());

                        if let Ok(new_graph) = crate::state_machine::build_state_graph(
                            &new_txs,
                            &address_clone,
                            schema_parser_clone.as_ref(),
                        ) && tx_sender.send((new_graph, new_txs)).await.is_err()
                        {
                            break; // Receiver closed
                        }
                    }
                }

                interval.tick().await;
            }
        });

        // Run TUI with receiver
        tracing::info!("Launching TUI in watch mode...");
        crate::tui::run(graph, transactions, Some(rx_receiver))?;

        Ok(())
    }
}

/// Schema validate command implementation
pub mod schema_validate {
    use super::*;
    use crate::parser::schema::ContractSchema;

    /// Execute the schema-validate command
    pub fn execute(schema_path: PathBuf) -> Result<()> {
        tracing::info!("Validating schema: {:?}", schema_path);

        // Load schema file
        let schema = match ContractSchema::from_file(&schema_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("❌ Failed to load schema: {}", e);
                return Err(e);
            }
        };

        // Validate structure
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if schema.contract.name.is_empty() {
            errors.push("Contract name cannot be empty");
        }
        if schema.contract.script_address.is_empty() {
            errors.push("Script address cannot be empty");
        } else if !schema.contract.script_address.starts_with("addr") {
            warnings.push("Script address should start with 'addr'");
        }
        if schema.datum.datum_type.is_empty() {
            errors.push("Datum type cannot be empty");
        }
        let mut field_names = std::collections::HashSet::new();
        for field in &schema.datum.fields {
            if !field_names.insert(&field.name) {
                errors.push("Duplicate field name");
            }
        }
        let mut redeemer_indices = std::collections::HashSet::new();
        for redeemer in &schema.redeemer {
            if !redeemer_indices.insert(redeemer.constructor_index) {
                errors.push("Duplicate redeemer constructor index");
            }
        }

        // Print validation report
        println!("📋 Schema Validation Report");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("File: {:?}", schema_path);
        println!();
        println!("Contract:");
        println!("  Name: {}", schema.contract.name);
        if let Some(ref desc) = schema.contract.description {
            println!("  Description: {}", desc);
        }
        println!("  Script Address: {}", schema.contract.script_address);
        println!();
        println!("Datum:");
        println!("  Type: {}", schema.datum.datum_type);
        println!("  Fields: {}", schema.datum.fields.len());
        for field in &schema.datum.fields {
            println!("    - {} ({})", field.name, field.field_type);
        }
        println!();
        println!("Redeemers: {}", schema.redeemer.len());
        for redeemer in &schema.redeemer {
            println!(
                "    - {} (index {})",
                redeemer.name, redeemer.constructor_index
            );
        }
        println!();

        // Print errors
        if !errors.is_empty() {
            println!("❌ Errors:");
            for error in &errors {
                println!("   {}", error);
            }
            println!();
        }

        // Print warnings
        if !warnings.is_empty() {
            println!("⚠️  Warnings:");
            for warning in &warnings {
                println!("   {}", warning);
            }
            println!();
        }

        // Final verdict
        if errors.is_empty() {
            println!("✅ Schema is valid!");
            Ok(())
        } else {
            println!("❌ Schema validation failed with {} error(s)", errors.len());
            Err(crate::Error::custom("Schema validation failed"))
        }
    }
}
