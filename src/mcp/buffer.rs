use tokio::sync::{mpsc, broadcast};
use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::RwLock;
use log::{debug, error};
use serde::{Serialize, Deserialize};
use std::time::{Duration, SystemTime};

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// A message in the buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferedMessage {
    /// Unique message ID
    pub id: uuid::Uuid,
    /// Message payload
    pub payload: Vec<u8>,
    /// Message priority
    pub priority: Priority,
    /// Timestamp when message was created
    pub created_at: SystemTime,
    /// Number of delivery attempts
    pub attempts: u32,
    /// Maximum number of delivery attempts
    pub max_attempts: u32,
    /// Optional delay before processing
    pub delay_until: Option<SystemTime>,
}

/// Configuration for the message buffer
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// Maximum buffer capacity
    pub capacity: usize,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Default message TTL
    pub message_ttl: Duration,
    /// Maximum delivery attempts
    pub max_attempts: u32,
    /// Cleanup interval
    pub cleanup_interval: Duration,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            capacity: 10000,
            max_message_size: 1024 * 1024, // 1MB
            message_ttl: Duration::from_secs(3600), // 1 hour
            max_attempts: 3,
            cleanup_interval: Duration::from_secs(60),
        }
    }
}

/// Message buffer with priority queue
#[derive(Debug)]
pub struct MessageBuffer {
    /// Internal priority queues
    queues: Arc<RwLock<Vec<VecDeque<BufferedMessage>>>>,
    /// Buffer configuration
    pub config: BufferConfig,
    /// Channel for publishing messages
    pub_tx: mpsc::Sender<BufferedMessage>,
    /// Channel for subscribing to messages
    sub_tx: broadcast::Sender<BufferedMessage>,
    /// Current buffer size
    size: Arc<RwLock<usize>>,
}

impl MessageBuffer {
    /// Create a new message buffer
    pub fn new(config: BufferConfig) -> Self {
        let (pub_tx, mut pub_rx) = mpsc::channel::<BufferedMessage>(config.capacity);
        let (sub_tx, _) = broadcast::channel::<BufferedMessage>(config.capacity);
        let sub_tx_clone = sub_tx.clone();
        
        // Initialize priority queues (one for each priority level)
        let queues = Arc::new(RwLock::new(vec![
            VecDeque::with_capacity(config.capacity), // Low
            VecDeque::with_capacity(config.capacity), // Normal
            VecDeque::with_capacity(config.capacity), // High
            VecDeque::with_capacity(config.capacity), // Critical
        ]));
        
        let size = Arc::new(RwLock::new(0));
        let queues_clone = queues.clone();
        let size_clone = size.clone();
        
        // Start message processor
        tokio::spawn(async move {
            while let Some(msg) = pub_rx.recv().await {
                let priority = msg.priority as usize;
                {
                    let mut queues = queues_clone.write();
                    queues[priority].push_back(msg.clone());
                    *size_clone.write() += 1;
                }
                
                if let Err(e) = sub_tx_clone.send(msg) {
                    error!("Failed to broadcast message: {}", e);
                }
            }
        });
        
        Self {
            queues,
            config,
            pub_tx,
            sub_tx,
            size,
        }
    }
    
    /// Get a subscriber for receiving messages
    pub fn subscribe(&self) -> broadcast::Receiver<BufferedMessage> {
        self.sub_tx.subscribe()
    }
    
    /// Publish a message to the buffer
    pub async fn publish(&self, msg: BufferedMessage) -> Result<(), String> {
        // Check message size
        if msg.payload.len() > self.config.max_message_size {
            return Err("Message exceeds maximum size".to_string());
        }

        // Check buffer capacity
        if self.len() >= self.config.capacity {
            return Err("Buffer is full".to_string());
        }

        // Publish to channel
        if let Err(e) = self.pub_tx.send(msg).await {
            error!("Failed to publish message: {}", e);
            return Err("Failed to publish message".to_string());
        }

        Ok(())
    }
    
    /// Pop a message from the specified priority queue
    pub fn pop(&self, priority: Priority) -> Option<BufferedMessage> {
        let mut queues = self.queues.write();
        let mut size = self.size.write();
        if let Some(msg) = queues[priority as usize].pop_front() {
            *size = size.saturating_sub(1);
            Some(msg)
        } else {
            None
        }
    }
    
    /// Pop the highest priority message available
    pub fn pop_any(&self) -> Option<BufferedMessage> {
        let mut queues = self.queues.write();
        let mut size = self.size.write();
        for queue in queues.iter_mut().rev() {  // Start from highest priority
            if let Some(msg) = queue.pop_front() {
                *size = size.saturating_sub(1);
                return Some(msg);
            }
        }
        None
    }
    
    /// Clean up expired messages
    pub async fn cleanup(&self) {
        let _now = SystemTime::now();
        let mut total_removed = 0;
        
        {
            let mut queues = self.queues.write();
            let mut size = self.size.write();
            for queue in queues.iter_mut() {
                let initial_len = queue.len();
                queue.retain(|msg| {
                    match msg.created_at.elapsed() {
                        Ok(elapsed) => elapsed < self.config.message_ttl,
                        Err(_) => false,
                    }
                });
                total_removed += initial_len - queue.len();
            }
            *size = size.saturating_sub(total_removed);
        }
        
        if total_removed > 0 {
            debug!("Cleaned up {} expired messages", total_removed);
        }
    }
    
    /// Get current buffer size
    pub fn len(&self) -> usize {
        *self.size.read()
    }
    
    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub async fn cleanup_expired(&mut self) {
        let _now = SystemTime::now();
        // TODO: Implement cleanup logic
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_basic_operations() {
        let buffer = MessageBuffer::new(BufferConfig::default());
        let msg = BufferedMessage {
            id: Uuid::new_v4(),
            payload: vec![1, 2, 3],
            priority: Priority::High,
            created_at: SystemTime::now(),
            attempts: 0,
            max_attempts: 3,
            delay_until: None,
        };

        // Test publish
        assert!(buffer.publish(msg.clone()).await.is_ok());

        // Test pop
        let received = buffer.pop(Priority::High);
        assert!(received.is_some());
        let received = received.unwrap();
        assert_eq!(received.id, msg.id);
        assert_eq!(received.priority, Priority::High);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let buffer = MessageBuffer::new(BufferConfig::default());

        // Create messages with different priorities
        let high_msg = BufferedMessage {
            id: Uuid::new_v4(),
            payload: vec![1],
            priority: Priority::High,
            created_at: SystemTime::now(),
            attempts: 0,
            max_attempts: 3,
            delay_until: None,
        };

        let low_msg = BufferedMessage {
            id: Uuid::new_v4(),
            payload: vec![2],
            priority: Priority::Low,
            created_at: SystemTime::now(),
            attempts: 0,
            max_attempts: 3,
            delay_until: None,
        };

        // Publish messages in reverse priority order
        assert!(buffer.publish(low_msg.clone()).await.is_ok());
        assert!(buffer.publish(high_msg.clone()).await.is_ok());

        // High priority message should be popped first
        let received = buffer.pop_any();
        assert!(received.is_some());
        let received = received.unwrap();
        assert_eq!(received.priority, Priority::High);

        // Low priority message should be popped second
        let received = buffer.pop_any();
        assert!(received.is_some());
        let received = received.unwrap();
        assert_eq!(received.priority, Priority::Low);
    }

    #[tokio::test]
    async fn test_cleanup() {
        let buffer = MessageBuffer::new(BufferConfig {
            cleanup_interval: Duration::from_millis(100),
            message_ttl: Duration::from_millis(50),
            ..Default::default()
        });

        let msg = BufferedMessage {
            id: Uuid::new_v4(),
            payload: vec![1],
            priority: Priority::High,
            created_at: SystemTime::now(),
            attempts: 0,
            max_attempts: 3,
            delay_until: None,
        };

        assert!(buffer.publish(msg).await.is_ok());

        // Wait for cleanup
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Message should be cleaned up
        assert!(buffer.pop(Priority::High).is_none());
    }
}