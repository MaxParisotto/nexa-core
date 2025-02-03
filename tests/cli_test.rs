use log::info;
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicU16, Ordering};
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

    // Create temporary directories
    let temp_dir = tempfile::tempdir().unwrap();
    let runtime_dir = temp_dir.path().to_path_buf();
    let pid_file = runtime_dir.join("nexa-test.pid");
    let socket_path = runtime_dir.join("nexa-test.sock");

    // Get a unique port for this test
    let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let addr = format!("127.0.0.1:{}", port);

    // Create a new CLI handler
    let cli = CliHandler::with_paths(pid_file, socket_path);

    // Test server status when not running
    info!("Testing server status when not running");
    assert!(!cli.is_server_running());

    // Start the server
    info!("Starting server");
    cli.start(Some(&addr)).await.expect("Failed to start server");
    assert!(cli.is_server_running());

    // Get server status
    info!("Getting server status");
    cli.status().await.expect("Failed to get server status");

    // Stop the server
    info!("Stopping server");
    cli.stop().await.expect("Failed to stop server");
    assert!(!cli.is_server_running());

    info!("CLI handler test completed successfully");
} 