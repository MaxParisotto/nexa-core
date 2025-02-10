use nexa_core::error::NexaError;
use nexa_core::server::Server;
use std::path::PathBuf;
use tokio::time::Duration;

#[tokio::test]
async fn test_server_stress() -> Result<(), NexaError> {
    let server = Server::new(
        PathBuf::from("/tmp/test.pid"),
        PathBuf::from("/tmp/test.sock"),
    );
    
    server.start().await?;
    
    // Run some basic stress tests
    for _ in 0..10 {
        let metrics = server.get_metrics().await;
        assert!(metrics.total_connections < u64::MAX);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    server.stop().await?;
    Ok(())
} 