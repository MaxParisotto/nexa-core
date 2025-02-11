use log::info;
use once_cell::sync::OnceCell;
use std::sync::atomic::AtomicU16;
use nexa_core::cli::CliHandler;
use std::fs;

#[allow(dead_code)]
static PORT_COUNTER: AtomicU16 = AtomicU16::new(9000);

static TRACING: OnceCell<()> = OnceCell::new();

/// Initialize tracing for tests
fn init_tracing() {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .try_init()
            .unwrap_or_default();
    });
}

#[tokio::test]
async fn test_cli_handler() {
    init_tracing();

    let temp_dir = tempfile::tempdir().unwrap();
    let pid_file = temp_dir.path().join("nexa.pid");
    let state_file = temp_dir.path().join("nexa.state");
    let socket_path = temp_dir.path().join("nexa.sock");

    // Create runtime directory
    fs::create_dir_all(temp_dir.path()).unwrap();

    let cli = CliHandler::with_paths(pid_file, state_file, socket_path);
    
    // Test server start
    assert!(!cli.is_server_running());
    assert!(cli.start(None).await.is_ok());
    
    // Wait for server to start (up to 5 seconds)
    let mut attempts = 0;
    while !cli.is_server_running() && attempts < 50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        attempts += 1;
    }
    assert!(cli.is_server_running(), "Server failed to start after 5 seconds");
    
    // Test server status
    assert!(cli.status().await.is_ok());
    
    // Test server stop
    assert!(cli.stop().await.is_ok());
    
    // Wait for server to stop (up to 5 seconds)
    let mut attempts = 0;
    while cli.is_server_running() && attempts < 50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        attempts += 1;
    }
    assert!(!cli.is_server_running(), "Server failed to stop after 5 seconds");

    info!("CLI handler test completed successfully");
} 