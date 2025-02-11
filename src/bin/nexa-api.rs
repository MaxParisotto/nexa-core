use std::net::SocketAddr;
use nexa_core::{
    api::ApiServer,
    cli::CliHandler,
};
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse};
use log::{info, error, LevelFilter};
use tracing::Level;
use axum::serve;
use tokio::net::TcpListener;

const API_VERSION: &str = "1.0.0";
const API_DESCRIPTION: &str = "Nexa Core API Server - REST API for managing Nexa Core server, agents, and workflows";

#[tokio::main]
async fn main() {
    // Initialize logging with more detailed configuration
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .filter_module("tower_http", LevelFilter::Debug)
        .filter_module("axum", LevelFilter::Debug)
        .init();

    // Create CLI handler
    let cli = CliHandler::new();

    // Create API server
    let api = ApiServer::new(cli);

    // Create router with enhanced middleware
    let app = api.router()
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .max_age(std::time::Duration::from_secs(3600)),
        )
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO))
        );

    // Bind to address
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Starting {} on {}", API_DESCRIPTION, addr);
    info!("API Version: {}", API_VERSION);
    info!("Documentation:");
    info!("  - Swagger UI: http://localhost:3000/swagger-ui/");
    info!("  - OpenAPI Spec: http://localhost:3000/api-docs/openapi.json");
    info!("Features enabled:");
    info!("  - CORS: ✓");
    info!("  - Request Tracing: ✓");
    info!("  - Interactive API Documentation: ✓");
    // Start server with improved error handling
    let listener = TcpListener::bind(addr).await.expect("Failed to bind to address");
    info!("Server listening on {}", addr);
    
    if let Err(err) = serve(listener, app).await {
        error!("Server error: {}", err);
        error!("Error details: {:?}", err);
        std::process::exit(1);
    }
    
    info!("Server stopped gracefully");
} 