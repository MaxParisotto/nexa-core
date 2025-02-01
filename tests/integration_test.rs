use nexa_utils::{
    error::NexaError,
    memory::{MemoryManager, ResourceType},
    monitoring::MonitoringSystem,
    tokens::{TokenManager, ModelType},
    mcp::{ServerControl, MCPMessage},
    agent::AgentStatus,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::SinkExt;
use std::time::Duration;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;
use std::path::PathBuf;
use url::Url;

async fn setup_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .with_thread_names(true)
        .with_level(true)
        .with_ansi(true)
        .try_init()
        .ok();
}

#[tokio::test]
async fn test_full_system_integration() -> Result<(), NexaError> {
    setup_logging().await;
    tracing::info!("Starting full system integration test");
    
    // Initialize components
    tracing::info!("Initializing components");
    let memory_manager = Arc::new(MemoryManager::new());
    let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
    let monitoring = Arc::new(MonitoringSystem::new(memory_manager.clone(), token_manager.clone()));
    
    // Set up server paths
    let runtime_dir = std::env::var("TMPDIR")
        .map(|dir| dir.trim_end_matches('/').to_string())
        .unwrap_or_else(|_| "/tmp".to_string());
    let runtime_dir = PathBuf::from(runtime_dir);
    let pid_file = runtime_dir.join("nexa-test-integration.pid");
    let socket_path = runtime_dir.join("nexa-test-integration.sock");
    
    let server = ServerControl::new(pid_file, socket_path);

    // Test memory management
    tracing::info!("Testing memory management");
    let metadata = HashMap::new();
    memory_manager.allocate("test-1".to_string(), ResourceType::TokenBuffer, 1024, metadata.clone()).await?;
    let stats = memory_manager.get_stats().await;
    assert!(stats.total_allocated > 0, "Expected total allocated memory to be positive");
    memory_manager.deallocate("test-1").await?;

    // Test memory edge cases
    tracing::info!("Testing memory edge cases");
    let mut handles = vec![];
    for i in 0..100 {
        let mm = memory_manager.clone();
        let metadata = metadata.clone();
        handles.push(tokio::spawn(async move {
            let id = format!("test-{}", i);
            mm.allocate(id.clone(), ResourceType::TokenBuffer, 1024, metadata).await?;
            mm.deallocate(&id).await?;
            Ok::<_, NexaError>(())
        }));
    }

    for handle in handles {
        handle.await.map_err(|e| NexaError::system(e.to_string()))??;
    }

    // Test token management
    tracing::info!("Testing token management");
    let mut metadata = HashMap::new();
    metadata.insert("test".to_string(), "value".to_string());
    token_manager.track_usage(ModelType::GPT4, 100, 50, metadata).await?;
    let usage = token_manager.get_usage_by_model(ModelType::GPT4).await;
    assert!(usage.total_tokens > 0);

    // Test monitoring system
    tracing::info!("Testing monitoring system");
    monitoring.start_monitoring(Duration::from_millis(100)).await?;
    sleep(Duration::from_millis(200)).await;
    
    let health = monitoring.check_health().await?;
    assert!(health.is_healthy, "Expected system to be healthy");
    let metrics = monitoring.collect_metrics(0).await?;
    assert!(!metrics.cpu_usage.is_nan(), "Expected CPU usage to be a valid number");

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
    let status_update = MCPMessage::StatusUpdate {
        agent_id: "test-agent".to_string(),
        status: AgentStatus::Running,
    };
    let msg = serde_json::to_string(&status_update)
        .map_err(|e| NexaError::system(e.to_string()))?;
    ws_stream.send(Message::Text(msg.into()))
        .await
        .map_err(|e| NexaError::system(e.to_string()))?;

    // Clean shutdown
    tracing::info!("Performing clean shutdown");
    ws_stream.close(None)
        .await
        .map_err(|e| NexaError::system(e.to_string()))?;

    tracing::info!("Integration test completed successfully");
    Ok(())
}

#[tokio::test]
async fn test_server_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    // Set up temporary paths for test
    let runtime_dir = std::env::var("TMPDIR")
        .map(|dir| dir.trim_end_matches('/').to_string())
        .unwrap_or_else(|_| "/tmp".to_string());
    let runtime_dir = PathBuf::from(runtime_dir);
    let pid_file = runtime_dir.join("nexa-test.pid");
    let socket_path = runtime_dir.join("nexa-test.sock");

    // Create server control with explicit paths
    let server = ServerControl::new(pid_file.clone(), socket_path);

    // Test server start
    server.start(Some("127.0.0.1:0")).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify server is running
    assert!(pid_file.exists(), "PID file should exist after server start");

    // Test server stop
    server.stop().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify server has stopped
    assert!(!pid_file.exists(), "PID file should be removed after server stop");

    Ok(())
}