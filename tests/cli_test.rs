use tracing::info;
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicU16, Ordering};
use nexa_core::cli::CliHandler;
use std::time::Duration;
use std::path::PathBuf;
use std::fs;

#[allow(dead_code)]
static PORT_COUNTER: AtomicU16 = AtomicU16::new(9000);

static TRACING: OnceCell<()> = OnceCell::new();

/// Initialize tracing for tests
fn init_tracing() {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_env_filter("debug")
            .try_init()
            .unwrap_or_default();
    });
}

/// Create a unique test directory
async fn create_test_dir() -> PathBuf {
    let temp_dir = tempfile::tempdir().unwrap();
    let runtime_dir = temp_dir.path().to_path_buf();
    // Create directory synchronously to ensure it exists
    fs::create_dir_all(&runtime_dir).expect("Failed to create test directory");
    runtime_dir
}

/// Wait for a condition with timeout and logging
async fn wait_for_condition<F, Fut>(mut condition: F, timeout: Duration, description: &str) -> bool 
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = tokio::time::Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(100));

    while start.elapsed() < timeout {
        if condition().await {
            info!("Condition satisfied: {}", description);
            return true;
        }
        interval.tick().await;
        if start.elapsed().as_secs() % 1 == 0 {
            info!("Still waiting for {}, elapsed: {:?}", description, start.elapsed());
        }
    }
    false
}

#[tokio::test]
async fn test_cli_handler() {
    // Run the test with a longer timeout
    tokio::time::timeout(Duration::from_secs(30), async {
        init_tracing();

        // Create temporary directories
        let runtime_dir = create_test_dir().await;
        info!("Created test directory at {:?}", runtime_dir);
        
        let pid_file = runtime_dir.join("nexa-test.pid");
        let socket_path = runtime_dir.join("nexa-test.sock");
        
        info!("Using PID file: {:?}", pid_file);
        info!("Using socket path: {:?}", socket_path);

        // Create a new CLI handler
        let cli = CliHandler::new_with_paths(pid_file.clone(), socket_path.clone());

        // Ensure cleanup on test exit
        let cleanup_pid = pid_file.clone();
        let cleanup_socket = socket_path.clone();
        let _cleanup = scopeguard::guard((), move |_| {
            // Use synchronous file operations for cleanup
            if cleanup_pid.exists() {
                let _ = fs::remove_file(&cleanup_pid);
            }
            if cleanup_socket.exists() {
                let _ = fs::remove_file(&cleanup_socket);
            }
        });

        // Get a unique port for this test
        let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
        let addr = format!("127.0.0.1:{}", port);
        info!("Using address: {}", addr);

        // Test server status when not running
        info!("Testing server status when not running");
        assert!(!cli.is_server_running().await);

        // Start the server
        info!("Starting server");
        cli.start(Some(&addr)).await.expect("Failed to start server");

        // Wait for server to be ready (up to 10 seconds)
        info!("Waiting for server to be ready");
        let server_ready = wait_for_condition(
            || async {
                cli.is_server_running().await
            },
            Duration::from_secs(10),
            "server to be ready"
        ).await;
        assert!(server_ready, "Server failed to start within timeout");

        // Get server status
        info!("Getting server status");
        cli.status().await.expect("Failed to get server status");

        // Stop the server
        info!("Stopping server");
        cli.stop().await.expect("Failed to stop server");

        // Wait for server to stop (up to 10 seconds)
        info!("Waiting for server to stop");
        let server_stopped = wait_for_condition(
            || async {
                !cli.is_server_running().await
            },
            Duration::from_secs(10),
            "server to stop"
        ).await;
        assert!(server_stopped, "Server failed to stop within timeout");

        // Verify server is fully stopped
        assert!(!cli.is_server_running().await, "Server should be stopped");
        assert!(!pid_file.exists(), "PID file should be removed");
        assert!(!socket_path.exists(), "Socket file should be removed");

        info!("CLI handler test completed successfully");
    }).await.expect("Test timed out after 30 seconds");
} 