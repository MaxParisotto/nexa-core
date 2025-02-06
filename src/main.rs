use clap::Parser;
use log::{error, info};
use std::sync::Arc;

use nexa_core::cli::{Cli, Commands, CliHandler};
use nexa_core::gui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let cli = Cli::parse();
    let handler = Arc::new(CliHandler::new());

    match cli.command {
        Some(Commands::Start) => handler.start(None).await?,
        Some(Commands::Stop) => handler.stop().await?,
        Some(Commands::Status) => handler.status().await?,
        Some(Commands::Gui) => {
            if let Err(e) = nexa_core::gui::app::main() {
                error!("Failed to run GUI: {}", e);
            }
        }
        None => {
            info!("No command specified, starting GUI...");
            if let Err(e) = nexa_core::gui::app::main() {
                error!("Failed to run GUI: {}", e);
            }
        }
    }

    Ok(())
} 