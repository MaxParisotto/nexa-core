use log::info;
use once_cell::sync::OnceCell;
use std::sync::atomic::AtomicU16;
use nexa_core::cli::CliHandler;

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

    let cli = CliHandler::with_paths(pid_file, state_file, socket_path);
    
    // Test server start
    assert!(!cli.is_server_running());
    assert!(cli.start(None).await.is_ok());
    assert!(cli.is_server_running());
    
    // Test server status
    assert!(cli.status().await.is_ok());
    
    // Test server stop
    assert!(cli.stop().await.is_ok());
    assert!(!cli.is_server_running());

    info!("CLI handler test completed successfully");
} 