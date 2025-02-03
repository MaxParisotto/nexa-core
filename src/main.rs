mod server;
mod error;
mod cli;
mod gui;

use clap::Parser;
use cli::CliHandler;
use error::NexaError;
use std::sync::Arc;
use log::{info, error, debug};
use env_logger::Env;
use iced::{Application, Settings};

#[tokio::main]
async fn main() -> Result<(), NexaError> {
    // Initialize logging with debug level and timestamp
    env_logger::Builder::from_env(Env::default().default_filter_or("debug"))
        .format_timestamp_millis()
        .init();

    let cli = cli::Cli::parse();
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

    match cli.command {
        Some(cli::Commands::Gui) => {
            // Initialize server if not running
            if !handler.is_server_running() {
                info!("Starting Nexa Core server...");
                handler.start(None).await?;
                
                // Give the server a moment to initialize
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                if !handler.is_server_running() {
                    error!("Server failed to start");
                    return Err(NexaError::System("Failed to start server".to_string()));
                }
                info!("Server started successfully");
            }

            info!("Launching GUI...");
            debug!("Creating server reference for GUI");
            let server = Arc::new(handler.get_server().clone());
            debug!("Server reference created");

            // Run GUI
            info!("Starting GUI event loop");
            let mut settings = Settings::with_flags(server);
            settings.window = iced::window::Settings {
                size: (800, 600),
                position: iced::window::Position::Centered,
                min_size: Some((400, 300)),
                ..Default::default()
            };

            if let Err(e) = gui::NexaGui::run(settings) {
                error!("GUI error: {}", e);
                return Err(NexaError::System(format!("Failed to start GUI: {}", e)));
            }

            info!("GUI closed, stopping server...");
            // Stop server on GUI exit
            if handler.is_server_running() {
                match handler.stop().await {
                    Ok(_) => info!("Server stopped successfully"),
                    Err(e) => error!("Failed to stop server gracefully: {}", e),
                }
            }
        }
        Some(cli::Commands::Start) => handler.start(None).await?,
        Some(cli::Commands::Stop) => handler.stop().await?,
        Some(cli::Commands::Status) => handler.status().await?,
        None => {
            println!("No command specified. Use --help for usage information.");
        }
    }

    Ok(())
} 