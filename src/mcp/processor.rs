use std::time::Duration;
use log::{debug, error, info};
use crate::error::NexaError;
use crate::mcp::buffer::{BufferedMessage, MessageBuffer};
use std::sync::Arc;
use tokio::sync::mpsc;
use num_cpus;

#[cfg(test)]
use {
    uuid::Uuid,
    crate::mcp::buffer::Priority,
};

/// Configuration for message processor
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    /// Number of worker threads
    pub worker_count: usize,
    /// Maximum retries for failed messages
    pub max_retries: u32,
    /// Delay between retries
    pub retry_delay: Duration,
    /// Processing timeout
    pub timeout: Duration,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            worker_count: num_cpus::get(),
            max_retries: 3,
            retry_delay: Duration::from_secs(5),
            timeout: Duration::from_secs(30),
        }
    }
}

/// Message processing result
#[derive(Debug)]
pub enum ProcessingResult {
    /// Message processed successfully
    Success,
    /// Message processing failed, should be retried
    RetryAfter(Duration),
    /// Message processing failed permanently
    Error(String),
}

/// Message processor handles the processing of buffered messages
pub struct MessageProcessor {
    config: ProcessorConfig,
    buffer: Arc<MessageBuffer>,
    workers: Vec<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<tokio::sync::mpsc::Sender<()>>,
}

impl MessageProcessor {
    /// Create a new message processor
    pub fn new(config: ProcessorConfig, buffer: Arc<MessageBuffer>) -> Self {
        Self {
            config,
            buffer,
            workers: Vec::new(),
            shutdown_tx: None,
        }
    }

    /// Start message processing
    pub async fn start(&mut self) -> Result<(), NexaError> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let shutdown_rx = Arc::new(tokio::sync::Mutex::new(shutdown_rx));

        // Spawn worker tasks
        for worker_id in 0..self.config.worker_count {
            let buffer = self.buffer.clone();
            let config = self.config.clone();
            let shutdown_rx = shutdown_rx.clone();

            let handle = tokio::spawn(async move {
                Self::worker_loop(worker_id, buffer, config, shutdown_rx).await;
            });

            self.workers.push(handle);
        }

        info!("Started {} message processor workers", self.config.worker_count);
        Ok(())
    }

    /// Stop message processing
    pub async fn stop(&mut self) -> Result<(), NexaError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
            for handle in self.workers.drain(..) {
                let _ = handle.await;
            }
        }
        Ok(())
    }

    /// Worker loop for processing messages
    async fn worker_loop(
        worker_id: usize,
        buffer: Arc<MessageBuffer>,
        _config: ProcessorConfig,
        shutdown_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<()>>>,
    ) {
        let mut shutdown_rx = shutdown_rx.lock().await;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    debug!("Worker {} shutting down", worker_id);
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Try to get next message
                    if let Some(msg) = buffer.pop_any().await {
                        // Process message
                        match Self::process_message(msg).await {
                            ProcessingResult::Success => {
                                debug!("Worker {} successfully processed message", worker_id);
                            }
                            ProcessingResult::Error(e) => {
                                error!("Worker {} failed to process message: {}", worker_id, e);
                            }
                            ProcessingResult::RetryAfter(delay) => {
                                debug!("Worker {} will retry message after {:?}", worker_id, delay);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Process a single message
    async fn process_message(msg: BufferedMessage) -> ProcessingResult {
        // Basic message processing logic
        if msg.attempts >= 3 {
            ProcessingResult::Error("Max attempts reached".to_string())
        } else if msg.payload.is_empty() {
            ProcessingResult::Error("Empty payload".to_string())
        } else {
            ProcessingResult::Success
        }
    }

    pub async fn run(&mut self) {
        loop {
            if let Some(msg) = self.buffer.pop_any().await {
                match Self::process_message(msg).await {
                    ProcessingResult::Success => {
                        debug!("Successfully processed message");
                    }
                    ProcessingResult::Error(e) => {
                        error!("Failed to process message: {}", e);
                    }
                    ProcessingResult::RetryAfter(delay) => {
                        debug!("Message will be retried after {:?}", delay);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

impl std::fmt::Debug for MessageProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageProcessor")
            .field("buffer", &self.buffer)
            .field("workers_count", &self.workers.len())
            .field("is_shutdown", &self.shutdown_tx.is_none())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use crate::mcp::buffer::Priority;

    #[tokio::test]
    async fn test_message_processing() {
        let buffer = Arc::new(MessageBuffer::new(Default::default()));
        let config = ProcessorConfig::default();
        let mut processor = MessageProcessor::new(config, buffer.clone());

        // Start processor
        processor.start().await.unwrap();

        // Create test messages
        let messages = vec![
            BufferedMessage {
                id: Uuid::new_v4(),
                payload: vec![1],
                priority: Priority::Critical,
                created_at: std::time::UNIX_EPOCH,
                attempts: 0,
                max_attempts: 3,
                delay_until: None,
            },
            BufferedMessage {
                id: Uuid::new_v4(),
                payload: vec![2],
                priority: Priority::High,
                created_at: std::time::UNIX_EPOCH,
                attempts: 0,
                max_attempts: 3,
                delay_until: None,
            },
        ];

        // Publish messages
        for msg in messages {
            buffer.publish(msg).await.unwrap();
        }

        // Wait for processing with a shorter timeout
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Stop processor and ensure it's stopped
        processor.stop().await.unwrap();
        
        // Add a small delay to ensure cleanup
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}