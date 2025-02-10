use super::*;
use tokio::time::timeout;
use std::time::Duration;

#[tokio::test]
async fn test_message_processing() {
    let config = ProcessorConfig {
        num_workers: 2,
        queue_size: 10,
        processing_timeout: Duration::from_secs(1),
    };
    
    let processor = MessageProcessor::new(config);
    let message = BufferedMessage {
        id: Uuid::new_v4(),
        payload: vec![1, 2, 3],
        priority: Priority::High,
        created_at: std::time::SystemTime::now(),
        attempts: 0,
        max_attempts: 3,
        delay_until: None,
    };

    // Add timeout to prevent test from hanging
    let result = timeout(
        Duration::from_secs(5),
        processor.process_message(message.clone())
    ).await;

    assert!(result.is_ok(), "Test timed out");
    if let Ok(process_result) = result {
        assert!(process_result.is_ok(), "Message processing failed");
    }

    // Ensure proper shutdown
    drop(processor);
    tokio::time::sleep(Duration::from_millis(100)).await;
} 