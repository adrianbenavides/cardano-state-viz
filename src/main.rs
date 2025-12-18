//! Cardano State Machine Visualizer

use cardano_state_viz::{Config, Result, VERSION, cli, init_logging};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Cli::parse();

    // Use default level, can be overridden by config
    init_logging("info");

    tracing::info!("Cardano State Machine Visualizer v{}", VERSION);
    tracing::debug!("Parsed arguments: {:?}", args);

    let config = if let Some(config_path) = &args.config {
        Config::from_file(config_path)?
    } else {
        Config::load()?
    };

    tracing::debug!("Loaded configuration: {:?}", config);

    cli::execute(args, config).await?;

    Ok(())
}
