use nexa_core::cli::run;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Run CLI handler
    run().await?;

    Ok(())
}