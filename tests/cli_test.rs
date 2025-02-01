use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info, error};
use once_cell::sync::OnceCell;
use uuid::Uuid;
use std::sync::atomic::{AtomicU16, Ordering};
use nexa_utils::mcp::server::ServerState;
use nexa_utils::error::NexaError;
use nexa_utils::cli::CliController;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use futures::{SinkExt, StreamExt};
use serde_json;

static PORT_COUNTER: AtomicU16 = AtomicU16::new(9000);

static TRACING: OnceCell<()> = OnceCell::new();

fn setup_logging() {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .with_thread_names(true)
            .with_level(true)
            .with_ansi(true)
            .try_init()
            .ok();
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
                    // Add additional delay to ensure server is fully ready
                    sleep(Duration::from_millis(500)).await;
                    
                    // Verify PID file exists
                    if !tokio::fs::try_exists(cli.get_pid_file_path()).await.unwrap_or(false) {
                        debug!("PID file missing after server start");
                        return Err(NexaError::system("PID file missing after server start"));
                    }
                    
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
    let mut retries = 100;  // Increased from 50
    let retry_delay = Duration::from_millis(200);
    
    while retries > 0 {
        match cli.get_server_state().await {
            Ok(state) => {
                debug!("Server state: {:?}", state);
                if state == ServerState::Stopped {
                    debug!("Server is stopped");
                    
                    // Verify PID file is removed
                    if tokio::fs::try_exists(cli.get_pid_file_path()).await.unwrap_or(false) {
                        debug!("PID file still exists after server stop");
                        let _ = tokio::fs::remove_file(cli.get_pid_file_path()).await;
                    }
                    
                    return Ok(());
                } else if state == ServerState::Stopping {
                    debug!("Server is in the process of stopping");
                }
            }
            Err(e) => {
                debug!("Error getting server state: {}", e);
                // If we can't get the state, the server might be already stopped
                if e.to_string().contains("Server is not running") {
                    debug!("Server appears to be stopped (not running)");
                    return Ok(());
                }
            }
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

async fn setup() -> Result<(CliController, String), NexaError> {
    setup_logging();
    
    let (pid_file, socket_path, state_file) = get_test_paths();
    
    // Create parent directories if they don't exist
    if let Some(parent) = pid_file.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| NexaError::system(format!("Failed to create parent directory: {}", e)))?;
    }
    if let Some(parent) = socket_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| NexaError::system(format!("Failed to create parent directory: {}", e)))?;
    }
    if let Some(parent) = state_file.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| NexaError::system(format!("Failed to create parent directory: {}", e)))?;
    }
    
    // Find an available port
    let port = find_available_port().await
        .ok_or_else(|| NexaError::system("Could not find available port"))?;
    
    // Use 127.0.0.1 for both server and client to ensure local connections work
    let addr = format!("127.0.0.1:{}", port);
    info!("Using address {} for server and client", addr);
    
    // Clean up any existing files
    let cleanup = async {
        if tokio::fs::try_exists(&pid_file).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(&pid_file).await;
        }
        if tokio::fs::try_exists(&socket_path).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(&socket_path).await;
        }
        if tokio::fs::try_exists(&state_file).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(&state_file).await;
        }
    };
    
    // Ensure cleanup is performed
    cleanup.await;
    
    let controller = CliController::new_with_paths(pid_file, socket_path, state_file);
    
    // Add delay to ensure file system operations are complete
    sleep(Duration::from_millis(100)).await;
    
    Ok((controller, addr))
}

async fn teardown() {
    // Clean up test files
    let temp_dir = std::env::temp_dir();
    
    // Read directory entries and remove test files
    if let Ok(entries) = std::fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with("nexa-test-") {
                    let _ = tokio::fs::remove_file(&path).await;
                }
            }
        }
    }
    
    // Give the OS time to release resources
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_cli_functionality() -> Result<(), NexaError> {
    let (cli, addr) = setup().await?;
    
    // Start server
    cli.handle_start(&Some(addr.clone())).await?;
    wait_for_server_start(&cli).await?;
    
    // Verify server is running
    assert_eq!(cli.get_server_state().await?, ServerState::Running);
    
    // Stop server
    cli.handle_stop().await?;
    wait_for_server_stop(&cli).await?;
    
    // Verify server is stopped
    match cli.get_server_state().await {
        Ok(state) => assert_eq!(state, ServerState::Stopped),
        Err(e) => {
            if !e.to_string().contains("Server is not running") {
                return Err(e);
            }
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_cli_resource_monitoring() {
    info!("Starting CLI resource monitoring test");
    let (cli, _) = setup().await.expect("Setup failed");
    
    // Test implementation
    let result = async {
        cli.handle_start(&None).await?;
        wait_for_server_start(&cli).await?;
        
        // Test metrics command
        let metrics = cli.handle_status().await?;
        assert!(metrics.contains("CPU:"), "Metrics should contain CPU usage");
        assert!(metrics.contains("Memory:"), "Metrics should contain memory usage");
        
        // Clean up
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
async fn test_cli_concurrent_connections() -> Result<(), NexaError> {
    let (cli, addr) = setup().await?;
    
    // Start server
    info!("Starting server with address: {}", addr);
    cli.handle_start(&Some(addr.clone())).await?;
    info!("Waiting for server to start");
    wait_for_server_start(&cli).await?;
    
    // Add additional delay to ensure WebSocket server is fully initialized
    info!("Adding delay for server initialization");
    sleep(Duration::from_secs(1)).await;
    
    // Verify server is running and ready
    let server_state = cli.get_server_state().await?;
    info!("Server state: {:?}", server_state);
    assert_eq!(server_state, ServerState::Running);
    
    // Verify server is accepting connections
    info!("Verifying server is accepting connections");
    let ws_url = format!("ws://{}", addr);
    let mut retries = 5;
    let mut connected = false;
    while retries > 0 && !connected {
        match connect_async(&ws_url).await {
            Ok((mut ws_stream, _)) => {
                info!("Successfully established test connection");
                ws_stream.close(None).await
                    .map_err(|e| NexaError::system(format!("Failed to close test connection: {}", e)))?;
                connected = true;
            }
            Err(e) => {
                error!("Test connection attempt failed (retries left: {}): {}", retries - 1, e);
                retries -= 1;
                if retries > 0 {
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
    
    if !connected {
        let _ = cli.handle_stop().await;
        let _ = wait_for_server_stop(&cli).await;
        return Err(NexaError::system("Failed to verify server is accepting connections"));
    }
    
    // Create multiple WebSocket connections
    let mut handles = Vec::new();
    for i in 0..3 {
        let addr = addr.clone();
        info!("Spawning connection task {}", i);
        let handle = tokio::spawn(async move {
            let mut retries = 5;
            let mut last_error = None;
            let ws_url = format!("ws://{}", addr);
            info!("Connection {}: Attempting to connect to {}", i, ws_url);
            
            while retries > 0 {
                info!("Connection {}: Attempt {} of 5", i, 6 - retries);
                match connect_async(&ws_url).await {
                    Ok((mut ws_stream, _)) => {
                        info!("Connection {}: Successfully established", i);
                        
                        // Send a test message
                        let test_msg = serde_json::json!({
                            "type": "test",
                            "connection_id": i,
                            "message": "Hello from test client"
                        });
                        info!("Connection {}: Sending test message: {}", i, test_msg);
                        ws_stream.send(Message::Text(test_msg.to_string()))
                            .await
                            .map_err(|e| NexaError::system(format!("Failed to send message: {}", e)))?;
                        
                        // Wait for response with timeout
                        info!("Connection {}: Waiting for response", i);
                        match tokio::time::timeout(Duration::from_secs(5), ws_stream.next()).await {
                            Ok(Some(Ok(msg))) => {
                                match msg {
                                    Message::Text(text) => {
                                        info!("Connection {}: Received response: {}", i, text);
                                        // Parse and verify response
                                        if let Ok(response) = serde_json::from_str::<serde_json::Value>(&text) {
                                            if response["status"] == "success" && response["code"] == 200 {
                                                info!("Connection {}: Response validation successful", i);
                                            } else {
                                                error!("Connection {}: Invalid response format: {}", i, text);
                                                return Err(NexaError::system(format!("Invalid response format: {}", text)));
                                            }
                                        } else {
                                            error!("Connection {}: Failed to parse response JSON: {}", i, text);
                                            return Err(NexaError::system(format!("Failed to parse response JSON: {}", text)));
                                        }
                                    }
                                    _ => {
                                        error!("Connection {}: Unexpected message type: {:?}", i, msg);
                                        return Err(NexaError::system(format!("Unexpected message type: {:?}", msg)));
                                    }
                                }
                            }
                            Ok(Some(Err(e))) => {
                                error!("Connection {}: Error receiving response: {}", i, e);
                                return Err(NexaError::system(format!("WebSocket error: {}", e)));
                            }
                            Ok(None) => {
                                error!("Connection {}: Connection closed unexpectedly", i);
                                return Err(NexaError::system("Connection closed unexpectedly"));
                            }
                            Err(_) => {
                                error!("Connection {}: Timeout waiting for response", i);
                                return Err(NexaError::system("Timeout waiting for response"));
                            }
                        }
                        
                        // Close connection gracefully
                        info!("Connection {}: Closing connection", i);
                        ws_stream.close(None)
                            .await
                            .map_err(|e| NexaError::system(format!("Failed to close connection: {}", e)))?;
                        
                        info!("Connection {}: Successfully completed", i);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("Connection {} attempt failed (retries left: {}): {}", i, retries - 1, e);
                        last_error = Some(e.to_string());
                        retries -= 1;
                        if retries > 0 {
                            info!("Connection {}: Waiting before retry", i);
                            sleep(Duration::from_millis(1000)).await;
                        }
                    }
                }
            }
            Err(NexaError::system(format!("Failed to connect after all retries. Last error: {:?}", last_error)))
        });
        handles.push(handle);
        // Add small delay between spawning connections
        sleep(Duration::from_millis(100)).await;
    }
    
    // Wait for all connections with improved error handling
    info!("Waiting for all connections to complete");
    let mut all_successful = true;
    let mut last_error = None;
    
    for (i, handle) in handles.into_iter().enumerate() {
        info!("Waiting for connection {} to complete", i);
        match handle.await {
            Ok(result) => {
                match result {
                    Ok(_) => info!("Connection {} completed successfully", i),
                    Err(e) => {
                        error!("Connection {} failed with error: {}", i, e);
                        all_successful = false;
                        last_error = Some(e);
                    }
                }
            }
            Err(e) => {
                error!("Task {} join error: {}", i, e);
                all_successful = false;
                last_error = Some(NexaError::system(format!("Task {} join error: {}", i, e)));
            }
        }
    }
    
    // Stop server
    info!("Stopping server");
    cli.handle_stop().await?;
    info!("Waiting for server to stop");
    wait_for_server_stop(&cli).await?;
    
    // Return result
    if all_successful {
        info!("Test completed successfully");
        Ok(())
    } else {
        Err(last_error.unwrap_or_else(|| NexaError::system("Test failed but no error was captured")))
    }
}

#[tokio::test]
async fn test_cli_error_handling() -> Result<(), NexaError> {
    let (cli, addr) = setup().await?;
    
    // Start server
    cli.handle_start(&Some(addr.clone())).await?;
    wait_for_server_start(&cli).await?;
    
    // Verify server is running
    assert_eq!(cli.get_server_state().await?, ServerState::Running);
    
    // Try to start server again (should fail)
    assert!(cli.handle_start(&Some(addr.clone())).await.is_err());
    
    // Stop server
    cli.handle_stop().await?;
    wait_for_server_stop(&cli).await?;
    
    // Try to stop server again (should fail)
    assert!(cli.handle_stop().await.is_err());
    
    Ok(())
}

trait TestCleanup {
    fn cleanup_files(&self) -> impl std::future::Future<Output = Result<(), NexaError>>;
}

impl TestCleanup for CliController {
    fn cleanup_files(&self) -> impl std::future::Future<Output = Result<(), NexaError>> {
        async move {
            // Get paths from controller
            let pid_file = self.get_pid_file_path();
            let socket_path = self.get_socket_path();
            let state_file = self.get_state_file_path();
            
            // Create parent directories if they don't exist
            if let Some(parent) = pid_file.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            
            // Remove PID file
            if tokio::fs::metadata(&pid_file).await.is_ok() {
                let _ = tokio::fs::remove_file(&pid_file).await;
            }
            
            // Remove socket file
            if tokio::fs::metadata(&socket_path).await.is_ok() {
                let _ = tokio::fs::remove_file(&socket_path).await;
            }
            
            // Remove state file
            if tokio::fs::metadata(&state_file).await.is_ok() {
                let _ = tokio::fs::remove_file(&state_file).await;
            }
            
            Ok(())
        }
    }
}