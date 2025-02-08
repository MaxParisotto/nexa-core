use clap::Parser;
use log::{error, info};
use nexa_core::cli::{Cli, Commands};
use std::path::PathBuf;
use std::process;

#[tokio::main]
async fn main() {
    // Initialize logging system
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nexa/logs");
    
    nexa_core::logging::init(log_dir.clone())
        .expect("Failed to initialize logger");

    info!("Starting Nexa Core v{}", env!("CARGO_PKG_VERSION"));
    info!("Log directory: {}", log_dir.display());

    let cli = Cli::parse();
    let cli_handler = nexa_core::cli::CliHandler::new();

    match cli.command {
        Some(Commands::Start) => {
            info!("Starting server...");
            if let Err(e) = cli_handler.start(None).await {
                error!("Failed to start server: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::Stop) => {
            info!("Stopping server...");
            if let Err(e) = cli_handler.stop().await {
                error!("Failed to stop server: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::Status) => {
            info!("Checking server status...");
            if let Err(e) = cli_handler.status().await {
                error!("Failed to get server status: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::Gui) => {
            info!("Starting GUI...");
            if let Err(e) = nexa_core::gui::app::main() {
                error!("Failed to start GUI: {}", e);
                process::exit(1);
            }
        }
        None => {
            info!("No command specified, starting GUI...");
            if let Err(e) = nexa_core::gui::app::main() {
                error!("Failed to start GUI: {}", e);
                process::exit(1);
            }
        }
    }
} 