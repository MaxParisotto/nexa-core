use nexa_utils::cli::CliController;
use nexa_utils::error::NexaError;
#[allow(unused_imports)]
use nexa_utils::mcp::MCPMessage;
#[allow(unused_imports)]
use nexa_utils::agent::AgentStatus;
use nexa_utils::mcp::server::ServerState;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, debug};
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicU16, Ordering};
use serde_json;
use std::path::PathBuf;
use uuid::Uuid;

static TRACING: OnceCell<()> = OnceCell::new();
static PORT_COUNTER: AtomicU16 = AtomicU16::new(9000);

fn setup_logging() {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_test_writer()
            .init();
    });
}

fn get_next_test_port() -> u16 {
    PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
}

fn get_test_paths() -> (PathBuf, PathBuf, PathBuf) {
    let runtime_dir = std::env::var("TMPDIR")
        .map(|dir| dir.trim_end_matches('/').to_string())
        .unwrap_or_else(|_| "/tmp".to_string());
    let runtime_dir = PathBuf::from(runtime_dir);
    let test_id = Uuid::new_v4();
    let pid_file = runtime_dir.join(format!("nexa-test-{}.pid", test_id));
    let socket_path = runtime_dir.join(format!("nexa-test-{}.sock", test_id));
    let state_file = pid_file.with_extension("state");
    (pid_file, socket_path, state_file)
}

async fn wait_for_server_start(cli: &CliController) -> Result<(), NexaError> {
    let mut retries = 70;
    let retry_delay = Duration::from_millis(200);
    
    while retries > 0 {
        match cli.get_server_state().await {
            Ok(state) => {
                debug!("Server state: {:?}", state);
                if state == ServerState::Running {
                    debug!("Server is running");
                    return Ok(());
                }
            }
            Err(e) => debug!("Error getting server state: {}", e),
        }
        sleep(retry_delay).await;
        retries -= 1;
        debug!("Waiting for server to start (retries left: {})", retries);
    }
    Err(NexaError::system("Server failed to start within timeout"))
}

async fn wait_for_server_stop(cli: &CliController) -> Result<(), NexaError> {
    let mut retries = 50;
    let retry_delay = Duration::from_millis(200);
    
    while retries > 0 {
        match cli.get_server_state().await {
            Ok(state) => {
                debug!("Server state: {:?}", state);
                if state == ServerState::Stopped {
                    debug!("Server is stopped");
                    return Ok(());
                }
            }
            Err(e) => debug!("Error getting server state: {}", e),
        }
        sleep(retry_delay).await;
        retries -= 1;
        debug!("Waiting for server to stop (retries left: {})", retries);
    }
    Err(NexaError::system("Server failed to stop within timeout"))
}

#[allow(dead_code)]
async fn cleanup_server(controller: &CliController) -> Result<(), NexaError> {
    let _ = controller.handle_stop().await;
    let _ = controller.cleanup_files().await;
    Ok(())
}

async fn setup_test() -> Result<(CliController, String), NexaError> {
    setup_logging();
    
    let (pid_file, socket_path, state_file) = get_test_paths();
    let port = get_next_test_port();
    let addr = format!("127.0.0.1:{}", port);
    
    // Clean up any existing files
    let _ = std::fs::remove_file(&pid_file);
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(&state_file);
    
    let controller = CliController::new_with_paths(pid_file, socket_path, state_file);
    Ok((controller, addr))
}

async fn setup() {
    // Clean up any stale files before starting tests
    let _ = std::fs::remove_file("/tmp/nexa.pid");
    let _ = std::fs::remove_file("/tmp/nexa.sock");
    let _ = std::fs::remove_file("/tmp/nexa.state");
}

async fn teardown() {
    // Clean up after tests
    let _ = std::fs::remove_file("/tmp/nexa.pid");
    let _ = std::fs::remove_file("/tmp/nexa.sock");
    let _ = std::fs::remove_file("/tmp/nexa.state");
    
    // Give the OS time to release the socket
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_cli_functionality() {
    info!("Starting CLI functionality test");
    setup().await;
    
    // Test implementation
    let result = async {
        let (cli, addr) = setup_test().await?;
        
        info!("Testing status command");
        let status = cli.handle_status().await?;
        assert!(status.contains("System Status"), "Status output missing system status");
        assert!(status.contains("Resource Usage"), "Status output missing resource usage");
        
        // Wait a bit longer for the server to be fully ready for WebSocket connections
        sleep(Duration::from_secs(1)).await;
        
        info!("Testing WebSocket connection");
        let url = format!("ws://{}", addr);
        debug!("Connecting to WebSocket at {}", url);
        
        // Add retry logic for WebSocket connection with exponential backoff
        let mut retries = 5;
        let mut socket = None;
        let mut delay = Duration::from_millis(100);
        
        while retries > 0 {
            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    debug!("Successfully established WebSocket connection");
                    socket = Some(ws_stream);
                    break;
                }
                Err(e) => {
                    debug!("WebSocket connection attempt failed: {}", e);
                    retries -= 1;
                    if retries > 0 {
                        sleep(delay).await;
                        delay *= 2; // Exponential backoff
                    }
                }
            }
        }
        
        let mut socket = socket.ok_or_else(|| NexaError::system("Failed to establish WebSocket connection"))?;
        
        // Send test message
        let test_msg = serde_json::json!({
            "type": "status",
            "agent_id": "test-cli-agent",
            "status": "Running"
        });
        
        debug!("Sending test message: {}", test_msg);
        socket.send(Message::Text(test_msg.to_string())).await?;
        
        // Wait for response with timeout
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            socket.next()
        ).await.map_err(|_| NexaError::system("Timeout waiting for WebSocket response"))?;
        
        // Verify response
        if let Some(response) = response {
            let response = response?;
            assert!(response.is_text(), "Response should be text");
            let response_text = response.into_text()?;
            debug!("Received response: {}", response_text);
            let response_json: serde_json::Value = serde_json::from_str(&response_text)?;
            assert_eq!(response_json["code"], 200, "Should receive success response");
        } else {
            return Err(NexaError::system("No response received from WebSocket"));
        }
        
        // Gracefully close the WebSocket connection
        debug!("Closing WebSocket connection");
        socket.close(None).await?;
        
        // Wait for connection to fully close
        sleep(Duration::from_millis(500)).await;
        
        info!("Testing stop command");
        let stop_result = cli.handle_stop().await;
        if let Err(e) = stop_result {
            if format!("{}", e).contains("Server is not running") {
                debug!("Stop command returned expected error: {}", e);
            } else {
                return Err(e);
            }
        }
        wait_for_server_stop(&cli).await?;
        
        Ok::<_, NexaError>(())
    }.await;
    
    // Ensure cleanup happens even if test fails
    teardown().await;
    
    // Now check the test result
    result.expect("Test failed");
}

#[tokio::test]
async fn test_cli_resource_monitoring() {
    info!("Starting CLI resource monitoring test");
    setup().await;
    
    // Test implementation
    let result = async {
        let cli = CliController::new();
        cli.handle_start(&None).await?;
        wait_for_server_start(&cli).await?;
        
        // Test monitoring functionality
        let metrics = cli.handle_status().await?;
        assert!(metrics.contains("System Status"), "Status output missing system status");
        assert!(metrics.contains("Resource Usage"), "Status output missing resource usage");
        
        cli.handle_stop().await?;
        wait_for_server_stop(&cli).await?;
        Ok::<_, NexaError>(())
    }.await;
    
    // Clean up regardless of test result
    teardown().await;
    
    // Now check the test result
    result.expect("Test failed");
}

#[tokio::test]
async fn test_cli_concurrent_connections() {
    info!("Starting CLI concurrent connections test");
    setup().await;
    
    // Test implementation
    let result = async {
        let cli = CliController::new();
        cli.handle_start(&None).await?;
        wait_for_server_start(&cli).await?;
        
        // Create multiple concurrent connections
        let bound_addr = cli.get_bound_addr().await?;
        let url = format!("ws://{}", bound_addr);
        let mut sockets = vec![];
        
        for i in 0..5 {
            let (mut socket, _) = connect_async(&url).await?;
            
            // Send test message
            let test_msg = serde_json::json!({
                "type": "status",
                "agent_id": format!("test-agent-{}", i),
                "status": "Running"
            });
            socket.send(Message::Text(test_msg.to_string())).await?;
            
            // Verify response
            if let Some(response) = socket.next().await {
                let response = response?;
                assert!(response.is_text(), "Response should be text");
                let response_text = response.into_text()?;
                let response_json: serde_json::Value = serde_json::from_str(&response_text)?;
                assert_eq!(response_json["code"], 200, "Should receive success response");
            }
            
            sockets.push(socket);
        }
        
        // Verify server handled all connections
        let metrics = cli.handle_status().await?;
        assert!(metrics.contains("Active Agents: 5"), "Should have 5 active connections");
        
        // Clean up connections
        for mut socket in sockets {
            let _ = socket.close(None).await;
        }
        
        cli.handle_stop().await?;
        wait_for_server_stop(&cli).await?;
        Ok::<_, NexaError>(())
    }.await;
    
    // Clean up regardless of test result
    teardown().await;
    
    // Now check the test result
    result.expect("Test failed");
}

#[tokio::test]
async fn test_cli_error_handling() {
    info!("Starting CLI error handling test");
    setup().await;
    
    // Test implementation
    let result = async {
        let cli = CliController::new();
        
        // Test starting server twice
        cli.handle_start(&None).await?;
        wait_for_server_start(&cli).await?;
        // Added delay to allow server fully settle into running state
        sleep(Duration::from_secs(1)).await;
        
        // Now, double start should reliably return an error
        assert!(cli.handle_start(&None).await.is_err(), "Should not be able to start server twice");
        
        // Test invalid address
        let cli2 = CliController::new();
        assert!(cli2.handle_start(&Some("invalid:address".to_string())).await.is_err(), "Should not accept invalid address");
        
        // Clean up
        let stop_result = cli.handle_stop().await;
        if let Err(e) = stop_result {
            if format!("{}", e).contains("Server is not running") {
                debug!("Stop command returned expected error: {}", e);
            } else {
                return Err(e);
            }
        }
        wait_for_server_stop(&cli).await?;
        Ok::<_, NexaError>(())
    }.await;
    
    // Clean up regardless of test result
    teardown().await;
    
    // Now check the test result
    result.expect("Test failed");
}