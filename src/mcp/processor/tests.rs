use super::*;
use tokio::time::timeout;
use std::time::Duration;
use crate::mcp::buffer::{BufferedMessage, MessageBuffer, Priority};
use std::time::SystemTime;
use std::sync::Arc;

#[tokio::test]
async fn test_message_processing() {
    // Create buffer with default config
    let buffer = Arc::new(MessageBuffer::new(Default::default()));
    
    // Create processor config
    let config = ProcessorConfig {
        worker_count: 1,
        max_retries: 3,
        retry_delay: Duration::from_millis(100),
        timeout: Duration::from_millis(500),
    };
    
    // Create processor
    let mut processor = MessageProcessor::new(config, buffer.clone());
    
    // Start processor
    processor.start().await.unwrap();
    
    // Create test message
    let message = BufferedMessage {
        id: Uuid::new_v4(),
        payload: vec![1, 2, 3],
        priority: Priority::High,
        created_at: SystemTime::now(),
        attempts: 0,
        max_attempts: 3,
        delay_until: None,
    };

    // Publish message to buffer
    buffer.publish(message.clone()).await.unwrap();

    // Wait a bit for processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Stop processor with timeout
    let stop_result = timeout(
        Duration::from_secs(1),
        processor.stop()
    ).await;

    assert!(stop_result.is_ok(), "Processor stop timed out");
    if let Ok(result) = stop_result {
        assert!(result.is_ok(), "Failed to stop processor cleanly");
    }
} 