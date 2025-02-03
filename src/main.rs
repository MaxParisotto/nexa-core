mod server;
mod error;
mod cli;
mod gui;
pub mod types;

use clap::Parser;
use cli::CliHandler;
use error::NexaError;
use std::sync::Arc;
use log::{info, error};
use env_logger::Env;

fn main() -> Result<(), NexaError> {
    // Initialize logging with debug level and timestamp
    env_logger::Builder::from_env(Env::default().default_filter_or("debug"))
        .format_timestamp_millis()
        .init();

    let cli = cli::Cli::parse();

    match cli.command {
        Some(cli::Commands::Gui) => {
            info!("Launching GUI...");
            let handler = Arc::new(CliHandler::new());
            
            // Run GUI on the main thread
            info!("Starting GUI event loop");
            if let Err(e) = gui::run_gui(handler) {
                error!("GUI error: {}", e);
                return Err(NexaError::System(format!("Failed to start GUI: {}", e)));
            }
        }
        _ => {
            // Create a tokio runtime for non-GUI commands
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| NexaError::System(format!("Failed to create runtime: {}", e)))?;
            
            let handler = Arc::new(CliHandler::new());
            let handler_clone = handler.clone();

            // Setup signal handler for cleanup
            ctrlc::set_handler(move || {
                let handler = handler_clone.clone();
                tokio::runtime::Runtime::new().unwrap().block_on(async move {
                    if handler.is_server_running() {
                        if let Err(e) = handler.stop().await {
                            error!("Error stopping server during cleanup: {}", e);
                        }
                    }
                    std::process::exit(0);
                });
            })?;

            // Run the command in the runtime
            rt.block_on(async {
                match cli.command {
                    Some(cli::Commands::Start) => handler.start(None).await,
                    Some(cli::Commands::Stop) => handler.stop().await,
                    Some(cli::Commands::Status) => handler.status().await,
                    _ => {
                        println!("No command specified. Use --help for usage information.");
                        Ok(())
                    }
                }
            })?;
        }
    }

    Ok(())
} 