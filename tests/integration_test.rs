use nexa_core::mcp::server::*;
use std::path::PathBuf;
use std::time::Duration;
use tracing::info;

#[tokio::test]
async fn test_server_startup() {
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

    let server = Server::new(pid_file.clone(), socket_path.clone());
    assert!(server.start().await.is_ok());
    info!("Server started successfully");

    // Wait for server to be ready
    tokio::time::sleep(Duration::from_secs(1)).await;

    let state = server.get_state().await;
    assert_eq!(state, ServerState::Running);

    // Clean up
    assert!(server.stop().await.is_ok());
    let _ = std::fs::remove_file(&pid_file);
    let _ = std::fs::remove_file(&socket_path);
}