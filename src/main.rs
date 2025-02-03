mod server;
mod error;
mod cli;
mod gui;

use clap::Parser;
use eframe::egui;
use cli::CliHandler;
use error::NexaError;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), NexaError> {
    env_logger::init();

    let cli = cli::Cli::parse();
    let handler = Arc::new(CliHandler::new());
    let handler_clone = handler.clone();

    // Setup signal handler for cleanup
    ctrlc::set_handler(move || {
        let handler = handler_clone.clone();
        tokio::runtime::Runtime::new().unwrap().block_on(async move {
            if handler.is_server_running() {
                if let Err(e) = handler.stop().await {
                    eprintln!("Error stopping server during cleanup: {}", e);
                }
            }
            std::process::exit(0);
        });
    })?;

    match cli.command {
        Some(cli::Commands::Gui) => {
            // Initialize server if not running
            if !handler.is_server_running() {
                handler.start(None).await?;
            }

            // GUI options
            let options = eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_inner_size([800.0, 600.0])
                    .with_min_inner_size([400.0, 300.0])
                    .with_title("Nexa Core")
                    .with_resizable(true),
                ..Default::default()
            };

            // Create GUI app with server reference
            let server = Arc::new(handler.get_server().clone());
            let app = gui::NexaApp::new(server);

            // Run GUI
            eframe::run_native(
                "Nexa Core",
                options,
                Box::new(move |_cc| Ok(Box::new(app))),
            ).map_err(|e| NexaError::System(e.to_string()))?;

            // Stop server on GUI exit
            if handler.is_server_running() {
                handler.stop().await?;
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