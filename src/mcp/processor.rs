#![allow(dead_code)]

use std::time::Duration;
use log::{debug, error, info};
use crate::error::NexaError;
use crate::mcp::buffer::{BufferedMessage, MessageBuffer};
use std::sync::Arc;
use tokio::sync::mpsc;
use num_cpus;
use futures;

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
    shutdown_tx: Option<mpsc::Sender<()>>,
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
            // Send shutdown signal
            let _ = tx.send(()).await;
            
            // Wait for all workers with timeout
            let mut handles = Vec::new();
            for handle in self.workers.drain(..) {
                handles.push(handle);
            }

            // Wait for all workers to complete with a timeout
            let timeout_duration = Duration::from_secs(1);
            let results = futures::future::join_all(
                handles.into_iter().map(|handle| {
                    tokio::time::timeout(timeout_duration, handle)
                })
            ).await;

            // Log any workers that failed to stop in time
            for (i, result) in results.iter().enumerate() {
                match result {
                    Ok(Ok(_)) => debug!("Worker {} stopped successfully", i + 1),
                    Ok(Err(e)) => error!("Worker {} failed: {}", i + 1, e),
                    Err(_) => error!("Worker {} failed to stop within timeout", i + 1),
                }
            }
        }
        Ok(())
    }

    /// Worker loop for processing messages
    async fn worker_loop(
        worker_id: usize,
        buffer: Arc<MessageBuffer>,
        config: ProcessorConfig,
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
                        // Process message with timeout
                        match tokio::time::timeout(
                            config.timeout,
                            Self::process_message(msg.clone())
                        ).await {
                            Ok(result) => {
                                match result {
                                    ProcessingResult::Success => {
                                        debug!("Worker {} successfully processed message", worker_id);
                                    }
                                    ProcessingResult::Error(e) => {
                                        error!("Worker {} failed to process message: {}", worker_id, e);
                                        if msg.attempts < config.max_retries {
                                            // Retry message after delay
                                            let mut retry_msg = msg;
                                            retry_msg.attempts += 1;
                                            retry_msg.delay_until = Some(
                                                std::time::SystemTime::now()
                                                    .checked_add(config.retry_delay)
                                                    .unwrap_or_else(|| std::time::SystemTime::now())
                                            );
                                            if let Err(e) = buffer.publish(retry_msg).await {
                                                error!("Failed to requeue message: {}", e);
                                            }
                                        }
                                    }
                                    ProcessingResult::RetryAfter(delay) => {
                                        debug!("Worker {} will retry message after {:?}", worker_id, delay);
                                        let mut retry_msg = msg;
                                        retry_msg.delay_until = Some(
                                            std::time::SystemTime::now()
                                                .checked_add(delay)
                                                .unwrap_or_else(|| std::time::SystemTime::now())
                                        );
                                        if let Err(e) = buffer.publish(retry_msg).await {
                                            error!("Failed to requeue message: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                error!("Worker {} message processing timed out", worker_id);
                                // Handle timeout - maybe retry message
                                if msg.attempts < config.max_retries {
                                    let mut retry_msg = msg;
                                    retry_msg.attempts += 1;
                                    if let Err(e) = buffer.publish(retry_msg).await {
                                        error!("Failed to requeue timed out message: {}", e);
                                    }
                                }
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
}

impl std::fmt::Debug for MessageProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageProcessor")
            .field("config", &self.config)
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
    use std::time::UNIX_EPOCH;

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
                created_at: UNIX_EPOCH,
                attempts: 0,
                max_attempts: 3,
                delay_until: None,
            },
            BufferedMessage {
                id: Uuid::new_v4(),
                payload: vec![2],
                priority: Priority::High,
                created_at: UNIX_EPOCH,
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