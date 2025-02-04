use iced::{Application, Settings};

pub mod app;
pub mod components;
pub mod styles;
pub mod types;
pub mod utils;

use app::NexaGui;
use std::sync::Arc;
use crate::cli::CliHandler;
use log::info;

pub fn run(handler: Arc<CliHandler>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Nexa GUI...");
    let settings = Settings::with_flags(handler);
    Ok(NexaGui::run(settings)?)
} 