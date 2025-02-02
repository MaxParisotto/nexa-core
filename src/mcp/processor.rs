use std::time::{Duration, SystemTime};
use tracing::{debug, error, info};
use crate::error::NexaError;
use crate::mcp::buffer::{BufferedMessage, MessageBuffer, Priority};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

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
    Failed(String),
}

/// Message processor handles the processing of buffered messages
pub struct MessageProcessor {
    config: ProcessorConfig,
    buffer: Arc<MessageBuffer>,
    workers: Vec<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    shutdown_rx: watch::Receiver<bool>,
}

impl MessageProcessor {
    /// Create a new message processor
    pub fn new(config: ProcessorConfig, buffer: Arc<MessageBuffer>, shutdown_rx: watch::Receiver<bool>) -> Self {
        Self {
            config,
            buffer,
            workers: Vec::new(),
            shutdown_tx: None,
            shutdown_rx,
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
            let shutdown_signal = self.shutdown_rx.clone();

            let handle = tokio::spawn(async move {
                Self::worker_loop(worker_id, buffer, config, shutdown_rx, shutdown_signal).await;
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
        config: ProcessorConfig,
        shutdown_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<()>>>,
        shutdown_signal: watch::Receiver<bool>,
    ) {
        let mut shutdown_rx = shutdown_rx.lock().await;

        loop {
            // Check if shutdown was signaled.
            if *shutdown_signal.borrow() {
                debug!("Worker {} received shutdown signal.", worker_id);
                break;
            }

            tokio::select! {
                _ = shutdown_rx.recv() => {
                    debug!("Worker {} shutting down", worker_id);
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Try to get next message, starting with highest priority
                    if let Some(msg) = buffer.pop_any() {
                        match Self::process_message(msg.clone()).await {
                            ProcessingResult::Success => {
                                debug!("Worker {} successfully processed message {}", worker_id, msg.id);
                            }
                            ProcessingResult::RetryAfter(delay) => {
                                if msg.attempts < config.max_retries {
                                    let mut retry_msg = msg;
                                    retry_msg.attempts += 1;
                                    retry_msg.delay_until = Some(SystemTime::now() + delay);
                                    
                                    if let Err(e) = buffer.publish(retry_msg).await {
                                        error!("Failed to requeue message: {}", e);
                                    }
                                } else {
                                    error!("Message {} exceeded retry limit", msg.id);
                                }
                            }
                            ProcessingResult::Failed(reason) => {
                                error!("Failed to process message {}: {}", msg.id, reason);
                            }
                        }
                    }
                }
            }
        }
        debug!("Worker {} exiting.", worker_id);
    }

    /// Process a single message
    async fn process_message(msg: BufferedMessage) -> ProcessingResult {
        // TODO: Implement actual message processing logic
        // This is a placeholder implementation
        match msg.priority {
            Priority::Critical => {
                // Process critical messages immediately
                ProcessingResult::Success
            }
            Priority::High => {
                // Simulate some processing
                tokio::time::sleep(Duration::from_millis(100)).await;
                ProcessingResult::Success
            }
            Priority::Normal => {
                // Simulate occasional retry
                if msg.attempts == 0 {
                    ProcessingResult::RetryAfter(Duration::from_secs(1))
                } else {
                    ProcessingResult::Success
                }
            }
            Priority::Low => {
                // Simulate longer processing
                tokio::time::sleep(Duration::from_millis(200)).await;
                ProcessingResult::Success
            }
        }
    }

    /// Check if the processor is running
    pub fn is_running(&self) -> bool {
        self.shutdown_tx.is_some() && !self.workers.is_empty()
    }
}

impl std::fmt::Debug for MessageProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageProcessor")
            .field("config", &self.config)
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
    use tokio::sync::watch;

    #[tokio::test]
    async fn test_message_processing() {
        let buffer = Arc::new(MessageBuffer::new(Default::default()));
        let config = ProcessorConfig::default();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let mut processor = MessageProcessor::new(config, buffer.clone(), shutdown_rx);

        // Start processor
        processor.start().await.unwrap();

        // Create test messages
        let messages = vec![
            BufferedMessage {
                id: Uuid::new_v4(),
                payload: vec![1],
                priority: Priority::Critical,
                created_at: SystemTime::now(),
                attempts: 0,
                max_attempts: 3,
                delay_until: None,
            },
            BufferedMessage {
                id: Uuid::new_v4(),
                payload: vec![2],
                priority: Priority::High,
                created_at: SystemTime::now(),
                attempts: 0,
                max_attempts: 3,
                delay_until: None,
            },
        ];

        // Publish messages
        for msg in messages {
            buffer.publish(msg).await.unwrap();
        }

        // Wait for processing
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Signal shutdown
        let _ = shutdown_tx.send(true);

        // Stop processor
        processor.stop().await.unwrap();
    }
}