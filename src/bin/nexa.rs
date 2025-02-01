use nexa_core::cli::CliController;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Create and run CLI controller
    let controller = CliController::new();
    controller.run().await?;

    Ok(())
}