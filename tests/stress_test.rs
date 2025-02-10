use nexa_core::error::NexaError;
use nexa_core::mcp::server::Server;
use tokio::time::Duration;
use tokio_tungstenite::{connect_async};
use tokio_tungstenite::tungstenite::Message;
use futures::sink::SinkExt;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_server_stress() -> Result<(), NexaError> {
    let temp_dir = tempfile::tempdir().unwrap();
    let pid_file = temp_dir.path().join("stress-test.pid");
    let socket_path = temp_dir.path().join("stress-test.sock");
    
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

    // Wait for server to be ready and get bound address
    tokio::time::sleep(Duration::from_millis(100)).await;
    let bound_addr = server.get_bound_addr().await.expect("Server should be bound to an address");
    let ws_url = format!("ws://{}", bound_addr);

    // Run stress tests with timeout
    let stress_timeout = tokio::time::timeout(
        Duration::from_secs(5),
        async {
            let mut handles = vec![];
            
            // Create multiple concurrent connections
            for i in 0..5 {
                let ws_url = ws_url.clone();
                let handle = tokio::spawn(async move {
                    if let Ok((mut ws_stream, _)) = connect_async(&ws_url).await {
                        // Send a test message
                        let msg = serde_json::json!({
                            "type": "test",
                            "id": i
                        });
                        let _ = ws_stream.send(Message::Text(msg.to_string())).await;
                        
                        // Wait a bit
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        
                        // Close connection gracefully
                        let _ = ws_stream.close(None).await;
                    }
                });
                handles.push(handle);
            }

            // Wait for all connections to complete
            for handle in handles {
                let _ = handle.await;
            }
        }
    ).await;
    
    assert!(stress_timeout.is_ok(), "Stress test timed out");
    
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
    
    Ok(())
} 