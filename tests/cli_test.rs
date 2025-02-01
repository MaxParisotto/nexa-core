use std::path::PathBuf;
use std::time::Duration;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info};
use once_cell::sync::OnceCell;
use tempfile;
use uuid::Uuid;
use futures::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicU16, Ordering};
use nexa_utils::mcp::server::{Server, ServerState};
use nexa_utils::error::NexaError;
use nexa_utils::cli::CliController;

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
    // Start from a random port in the dynamic range (49152-65535)
    static START_PORT: AtomicU16 = AtomicU16::new(49152);
    START_PORT.fetch_add(1, Ordering::SeqCst)
}

async fn find_available_port() -> Option<u16> {
    for _ in 0..100 {  // Try up to 100 times
        let port = get_next_test_port();
        let addr = format!("127.0.0.1:{}", port);
        if let Ok(listener) = tokio::net::TcpListener::bind(&addr).await {
            drop(listener);
            return Some(port);
        }
    }
    None
}

fn get_test_paths() -> (PathBuf, PathBuf, PathBuf) {
    let runtime_dir = std::env::temp_dir();
    let test_id = Uuid::new_v4();
    let base_name = format!("nexa-test-{}", test_id);
    
    let pid_file = runtime_dir.join(format!("{}.pid", base_name));
    let socket_path = runtime_dir.join(format!("{}.sock", base_name));
    let state_file = runtime_dir.join(format!("{}.state", base_name));
    
    // Create parent directories if they don't exist
    if let Some(parent) = pid_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    
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
    let _ = controller.cleanup_files();
    Ok(())
}

async fn setup_test() -> Result<(CliController, String), NexaError> {
    setup_logging();
    
    let (pid_file, socket_path, state_file) = get_test_paths();
    
    // Find an available port
    let port = find_available_port().await
        .ok_or_else(|| NexaError::system("Could not find available port"))?;
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
    let temp_dir = std::env::temp_dir();
    let _ = std::fs::remove_dir_all(temp_dir.join("nexa-test-*"));
    
    // Reset port counter to avoid conflicts
    PORT_COUNTER.store(9000, Ordering::SeqCst);
    
    // Give the OS time to release resources
    sleep(Duration::from_millis(100)).await;
}

async fn teardown() {
    // Clean up test files
    let temp_dir = std::env::temp_dir();
    let _ = std::fs::remove_dir_all(temp_dir.join("nexa-test-*"));
    
    // Give the OS time to release resources
    sleep(Duration::from_millis(500)).await;
}

#[tokio::test]
async fn test_cli_functionality() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let pid_file = temp_dir.path().join("nexa-test.pid");
    let socket_path = temp_dir.path().join("nexa-test.sock");
    let state_file = temp_dir.path().join("nexa-test.state");
    
    let server = Server::new(pid_file.clone(), socket_path.clone());
    let server_clone = server.clone();
    
    // Start server with random port
    tokio::spawn(async move {
        if let Err(e) = server_clone.start_server().await {
            error!("Server error: {}", e);
        }
    });
    
    // Wait for server to start
    let mut retries = 10;
    while retries > 0 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if server.get_state().await == ServerState::Running {
            break;
        }
        retries -= 1;
    }
    assert_eq!(server.get_state().await, ServerState::Running, "Server failed to start");
    
    // Get bound address
    let bound_addr = server.get_bound_addr().await.ok_or_else(|| Box::<dyn std::error::Error>::from("Failed to get bound address"))?;
    assert!(bound_addr.port() > 0, "Server should be bound to a valid port");
    
    // Test WebSocket connection
    info!("Testing WebSocket connection");
    let url = format!("ws://127.0.0.1:{}", bound_addr.port());
    debug!("Connecting to WebSocket at {}", url);
    
    let mut retries = 5;
    let mut last_error = None;
    
    while retries > 0 {
        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                let (mut write, mut read) = ws_stream.split();
                let message = serde_json::json!({
                    "type": "status",
                    "agent_id": "test-agent",
                    "status": "Running"
                });
                write.send(Message::Text(message.to_string())).await.map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
                
                if let Some(msg) = read.next().await {
                    let msg = msg.map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
                    assert!(msg.is_text(), "Response should be text");
                }
                break;
            }
            Err(e) => {
                debug!("WebSocket connection attempt failed: {}", e);
                last_error = Some(e);
                tokio::time::sleep(Duration::from_millis(100)).await;
                retries -= 1;
            }
        }
    }
    
    if retries == 0 {
        return Err(Box::<dyn std::error::Error>::from(format!(
            "Failed to establish WebSocket connection: {}",
            last_error.unwrap()
        )));
    }
    
    // Stop server
    server.stop().await.map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
    
    // Wait for server to stop
    let mut retries = 10;
    while retries > 0 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if server.get_state().await == ServerState::Stopped {
            break;
        }
        retries -= 1;
    }
    assert_eq!(server.get_state().await, ServerState::Stopped, "Server failed to stop");
    
    // Verify cleanup
    assert!(!pid_file.exists(), "PID file should be removed after server stop");
    assert!(!socket_path.exists(), "Socket file should be removed after server stop");
    assert!(!state_file.exists(), "State file should be removed after server stop");
    
    Ok(())
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

trait TestCleanup {
    fn cleanup_files(&self) -> impl std::future::Future<Output = Result<(), NexaError>>;
}

impl TestCleanup for CliController {
    fn cleanup_files(&self) -> impl std::future::Future<Output = Result<(), NexaError>> {
        async move {
            // Implementation
            Ok(())
        }
    }
}