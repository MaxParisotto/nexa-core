use nexa_core::mcp::server::*;
use std::path::PathBuf;
use std::time::Duration;
use log::info;

#[tokio::test]
async fn test_server_startup() {
    // Enable logging for debugging
    let _ = env_logger::try_init();

    // Set up temporary paths for test
    let runtime_dir = std::env::var("TMPDIR")
        .map(|dir| dir.trim_end_matches('/').to_string())
        .unwrap_or_else(|_| "/tmp".to_string());
    let runtime_dir = PathBuf::from(runtime_dir);
    let pid_file = runtime_dir.join("nexa-test.pid");
    let socket_path = runtime_dir.join("nexa-test.sock");

    // Clean up any existing files
    let _ = std::fs::remove_file(&pid_file);
    let _ = std::fs::remove_file(&socket_path);

    // Create server with test configuration
    let server = Server::new(pid_file.clone(), socket_path.clone());
    
    // Start server with timeout
    let start_result = tokio::time::timeout(
        Duration::from_secs(5),
        server.start()
    ).await;
    
    assert!(start_result.is_ok(), "Server start timed out");
    if let Ok(result) = start_result {
        assert!(result.is_ok(), "Server failed to start: {:?}", result.err());
    }
    info!("Server started successfully");

    // Wait for server to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify server state
    let state = server.get_state().await;
    assert_eq!(state, ServerState::Running, "Server should be in Running state");

    // Stop server with timeout
    let stop_result = tokio::time::timeout(
        Duration::from_secs(5),
        server.stop()
    ).await;
    
    assert!(stop_result.is_ok(), "Server stop timed out");
    if let Ok(result) = stop_result {
        assert!(result.is_ok(), "Server failed to stop cleanly: {:?}", result.err());
    }

    // Clean up test files
    let _ = std::fs::remove_file(&pid_file);
    let _ = std::fs::remove_file(&socket_path);
}