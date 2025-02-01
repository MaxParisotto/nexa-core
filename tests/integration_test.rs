use nexa_core::{
    mcp::{
        server::*,
        protocol::{Message as ProtocolMessage, MessageType, MessagePayload, StatusUpdatePayload},
        tokens::{TokenManager, ModelType},
    },
    error::NexaError,
    monitoring::*,
    agent::AgentStatus,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use futures::SinkExt;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;
use std::path::PathBuf;
use url::Url;

#[tokio::test]
async fn test_system_integration() -> Result<(), NexaError> {
    // Set up logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .with_thread_names(true)
        .with_level(true)
        .init();

    // Set up temporary paths
    let runtime_dir = std::env::temp_dir();
    let pid_file = runtime_dir.join("nexa-test.pid");
    let socket_path = runtime_dir.join("nexa-test.sock");

    // Create server control
    let server = ServerControl::new(pid_file.clone(), socket_path.clone());

    // Test server operations
    tracing::info!("Testing server operations");
    let health = server.check_health().await?;
    assert!(health, "Expected system to be healthy");

    // Test MCP system
    tracing::info!("Testing MCP system");
    server.start(Some("127.0.0.1:0")).await?;
    sleep(Duration::from_millis(100)).await; // Give server time to start
    
    // Get bound address
    let server_addr = server.get_bound_addr().await?;
    
    // Test client connection
    tracing::info!("Testing WebSocket client connection");
    let url = Url::parse(&format!("ws://{}", server_addr))
        .map_err(|e| NexaError::system(e.to_string()))?;
    let (mut ws_stream, _) = connect_async(url)
        .await
        .map_err(|e| NexaError::system(e.to_string()))?;
    
    // Send test message
    tracing::info!("Sending test message");
    let status_update = ProtocolMessage::new(
        MessageType::StatusUpdate,
        "test-agent".to_string(),
        MessagePayload::StatusUpdate(StatusUpdatePayload {
            agent_id: "test-agent".to_string(),
            status: AgentStatus::Running,
            metrics: None,
        }),
    );
    let msg = serde_json::to_string(&status_update)
        .map_err(|e| NexaError::system(e.to_string()))?;
    ws_stream.send(WsMessage::Text(msg)).await
        .map_err(|e| NexaError::system(e.to_string()))?;

    // Clean up
    server.stop().await?;
    let _ = tokio::fs::remove_file(pid_file).await;
    let _ = tokio::fs::remove_file(socket_path).await;

    Ok(())
}