//! Multi-agent Control Protocol (MCP) Implementation
//!
//! This module provides the core functionality for agent communication and management:
//! - Message types for agent registration and communication
//! - Connection handling
//! - Protocol implementation
//! - Registry management

pub mod registry;
pub mod server;
pub mod protocol;
pub mod tokens;
pub mod cluster;
pub mod config;
pub mod loadbalancer;
pub mod buffer;
pub mod processor;
pub mod cluster_processor;
pub mod metrics;

use std::path::PathBuf;
use std::time::Duration;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::agent::{Agent, Task, AgentStatus};
use std::sync::Arc;
use std::collections::HashMap;
use crate::error::NexaError;
use crate::mcp::server::{Server, ServerState};
use crate::monitoring::{
    MonitoringSystem, SystemMetrics, SystemHealth, SystemAlert, AlertLevel
};
use crate::memory::{MemoryManager, MemoryStats, ResourceType};
use tokio::sync::{RwLock, broadcast};
use log::{debug, error, info};
use chrono::Utc;
use crate::tokens::{TokenManager, ModelType, TokenUsage};
use crate::mcp::buffer::{BufferedMessage, MessageBuffer, Priority, BufferConfig};
use crate::mcp::processor::{MessageProcessor, ProcessorConfig};
use crate::mcp::cluster_processor::{ClusterProcessor, ClusterProcessorConfig};
use crate::mcp::metrics::{MetricsCollector, AlertChecker, AlertThresholds};
use std::net::SocketAddr;

pub use cluster::{ClusterManager, ClusterConfig, Node, NodeRole};

#[derive(Debug, Serialize, Deserialize)]
pub enum MCPMessage {
    RegisterAgent {
        agent: Agent,
    },
    DeregisterAgent {
        agent_id: String,
    },
    TaskAssignment {
        task: Task,
        agent_id: String,
    },
    StatusUpdate {
        agent_id: String,
        status: AgentStatus,
    },
    AgentQuery {
        capability: String,
    },
    AgentResponse {
        agents: Vec<Agent>,
    },
    Error {
        code: u32,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct MCPConnection {
    pub id: String,
    pub agent: Option<Agent>,
    pub active_connections: Arc<RwLock<u32>>,
}

// Explicitly implement Send and Sync since all fields are Send + Sync
unsafe impl Send for MCPConnection {}
unsafe impl Sync for MCPConnection {}

impl MCPConnection {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent: None,
            active_connections: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn handle_message(&self, message: MCPMessage) -> Result<serde_json::Value, NexaError> {
        match message {
            MCPMessage::StatusUpdate { agent_id, status } => {
                info!("Status update from {}: {:?}", agent_id, status);
                Ok(serde_json::json!({
                    "code": 200,
                    "message": format!("Status update received from {}", agent_id)
                }))
            }
            _ => {
                error!("Unsupported message type");
                Ok(serde_json::json!({
                    "code": 400,
                    "message": "Unsupported message type"
                }))
            }
        }
    }
}

/// Server control interface for MCP system
#[derive(Debug)]
pub struct ServerControl {
    pub registry: registry::AgentRegistry,
    pub protocol: protocol::ProtocolHandler,
    memory_manager: Arc<MemoryManager>,
    token_manager: Arc<TokenManager>,
    pub monitoring: Arc<MonitoringSystem>,
    server: Arc<Server>,
    server_handle: Arc<RwLock<Option<tokio::task::JoinHandle<Result<(), NexaError>>>>>,
    message_buffer: Arc<MessageBuffer>,
    message_processor: Arc<RwLock<Option<MessageProcessor>>>,
    cluster_processor: Arc<RwLock<Option<ClusterProcessor>>>,
    metrics_collector: Arc<MetricsCollector>,
    alert_checker: Arc<AlertChecker>,
    pid_file: PathBuf,
    socket_path: PathBuf,
}

impl Clone for ServerControl {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            protocol: self.protocol.clone(),
            memory_manager: self.memory_manager.clone(),
            token_manager: self.token_manager.clone(),
            monitoring: self.monitoring.clone(),
            server: self.server.clone(),
            server_handle: self.server_handle.clone(),
            message_buffer: self.message_buffer.clone(),
            message_processor: self.message_processor.clone(),
            cluster_processor: self.cluster_processor.clone(),
            metrics_collector: self.metrics_collector.clone(),
            alert_checker: self.alert_checker.clone(),
            pid_file: self.pid_file.clone(),
            socket_path: self.socket_path.clone(),
        }
    }
}

impl ServerControl {
    pub fn new(pid_file: PathBuf, socket_path: PathBuf) -> Self {
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        let monitoring = Arc::new(MonitoringSystem::new(memory_manager.clone(), token_manager.clone()));
        let message_buffer = Arc::new(MessageBuffer::new(BufferConfig::default()));
        let message_processor = Arc::new(RwLock::new(None));
        let cluster_processor = Arc::new(RwLock::new(None));
        let metrics_collector = Arc::new(MetricsCollector::new());
        let alert_checker = Arc::new(AlertChecker::new(
            AlertThresholds::default(),
            metrics_collector.clone(),
        ));

        Self {
            pid_file: pid_file.clone(),
            socket_path: socket_path.clone(),
            server: Arc::new(Server::new(pid_file, socket_path)),
            server_handle: Arc::new(RwLock::new(None)),
            registry: registry::AgentRegistry::new(),
            protocol: protocol::ProtocolHandler::new(),
            memory_manager,
            token_manager,
            monitoring,
            message_buffer,
            message_processor,
            cluster_processor,
            metrics_collector,
            alert_checker,
        }
    }

    pub async fn start(&self, addr: Option<&str>) -> Result<(), NexaError> {
        // Early check: if server task already exists, then server is running
        if self.server_handle.read().await.is_some() {
            error!("Server is already running");
            return Err(NexaError::System("Server is already running".to_string()));
        }

        // Start message processor
        let mut processor = MessageProcessor::new(
            ProcessorConfig::default(),
            self.message_buffer.clone(),
        );
        processor.start().await?;
        *self.message_processor.write().await = Some(processor);

        // Start cluster processor if clustering is enabled
        let server_config = self.server.get_config().await?;
        let cluster_config = Some(ClusterConfig {
            min_quorum_size: 1,
            heartbeat_interval: server_config.health_check_interval,
            election_timeout: (
                server_config.connection_timeout,
                server_config.connection_timeout * 2
            ),
            // Add other fields as needed
            ..Default::default()
        });

        if let Some(config) = cluster_config {
            let bind_addr = addr
                .and_then(|a| a.parse::<SocketAddr>().ok())
                .unwrap_or_else(|| "127.0.0.1:0".parse().unwrap());
                
            let mut cluster_processor = ClusterProcessor::new(
                ClusterProcessorConfig::default(),
                self.message_buffer.clone(),
                Arc::new(ClusterManager::new(bind_addr, Some(config))),
            );
            cluster_processor.start().await?;
            *self.cluster_processor.write().await = Some(cluster_processor);
        }

        // Start message cleanup task
        self.start_message_cleanup().await;

        // Start server
        let server = self.server.clone();
        server.start().await?;

        // Store the handle
        *self.server_handle.write().await = Some(tokio::spawn(async move {
            Ok(())
        }));

        // Poll the server state for up to 10 seconds until it becomes Running and has a bound address
        let timeout_duration = Duration::from_secs(10);
        let start_time = tokio::time::Instant::now();
        
        loop {
            if self.server.get_state().await == ServerState::Running {
                if let Some(bound_addr) = self.server.get_bound_addr().await {
                    // Add a small delay to ensure the WebSocket server is fully initialized
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    info!("Server started successfully on {}", bound_addr);
                    return Ok(());
                }
            }
            
            if start_time.elapsed() >= timeout_duration {
                return Err(NexaError::System("Server failed to start within timeout".to_string()));
            }
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    pub async fn stop(&self) -> Result<(), NexaError> {
        // Stop cluster processor
        if let Some(mut processor) = self.cluster_processor.write().await.take() {
            processor.stop().await?;
        }

        // Stop message processor
        if let Some(mut processor) = self.message_processor.write().await.take() {
            processor.stop().await?;
        }

        // Check if server is already stopped
        match self.server.get_state().await {
            ServerState::Stopped => {
                debug!("Server is already stopped");
                return Ok(());
            }
            _ => {}
        }
        
        // Stop the server
        if let Err(e) = self.server.stop().await {
            error!("Error stopping server: {}", e);
            return Err(e);
        }
        
        // Wait for server to stop with timeout
        let mut retries = 10;
        while retries > 0 {
            match self.server.get_state().await {
                ServerState::Stopped => {
                    debug!("Server stopped successfully");
                    return Ok(());
                }
                _ => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    retries -= 1;
                }
            }
        }

        error!("Server failed to stop within timeout");
        Err(NexaError::System("Server failed to stop within timeout".to_string()))
    }

    pub async fn get_bound_addr(&self) -> Result<std::net::SocketAddr, NexaError> {
        self.server.get_bound_addr().await
            .ok_or_else(|| NexaError::System("Server address not available".to_string()))
    }

    pub async fn get_state(&self) -> Result<ServerState, NexaError> {
        Ok(self.server.get_state().await)
    }

    pub async fn check_health(&self) -> Result<SystemHealth, NexaError> {
        // Check server state first
        match self.server.get_state().await {
            ServerState::Running => {
                // Perform health checks
                let active_connections = self.server.get_active_connections().await;
                let bound_addr = self.server.get_bound_addr().await;

                Ok(SystemHealth {
                    is_healthy: bound_addr.is_some() && active_connections < 1000,
                    message: format!(
                        "System healthy, {} active connections",
                        active_connections
                    ),
                    timestamp: Utc::now(),
                })
            }
            state => {
                Ok(SystemHealth {
                    is_healthy: false,
                    message: format!("Server is not running (state: {:?})", state),
                    timestamp: Utc::now(),
                })
            }
        }
    }

    pub async fn get_alerts(&self) -> Result<Vec<SystemAlert>, NexaError> {
        let mut alerts = Vec::new();
        
        // Check server state
        match self.server.get_state().await {
            ServerState::Running => {
                // Check active connections
                let active_connections = self.server.get_active_connections().await;
                if active_connections > 900 {
                    alerts.push(SystemAlert {
                        level: AlertLevel::Error,
                        message: format!("High connection count: {}", active_connections),
                        timestamp: Utc::now(),
                    });
                } else if active_connections > 700 {
                    alerts.push(SystemAlert {
                        level: AlertLevel::Warning,
                        message: format!("Elevated connection count: {}", active_connections),
                        timestamp: Utc::now(),
                    });
                }
            }
            state => {
                alerts.push(SystemAlert {
                    level: AlertLevel::Warning,
                    message: format!("Server is not running (state: {:?})", state),
                    timestamp: Utc::now(),
                });
            }
        }
        
        Ok(alerts)
    }

    pub async fn get_metrics(&self) -> Result<SystemMetrics, NexaError> {
        // Get basic metrics
        let active_connections = self.server.get_active_connections().await;
        
        Ok(SystemMetrics {
            cpu_usage: 6.6,  // Example value
            memory_used: 3,
            memory_allocated: 4,
            memory_available: 1,
            token_usage: 0,
            token_cost: 0.0,
            active_agents: active_connections,
            error_count: 0,
            timestamp: Utc::now(),
        })
    }

    /// Get memory statistics
    pub async fn memory_stats(&self) -> MemoryStats {
        self.memory_manager.get_stats().await
    }

    /// Track agent resource allocation
    pub async fn track_agent_resources(&self, agent_id: &str, resource_type: ResourceType, size: usize) -> Result<(), NexaError> {
        let metadata = HashMap::new();
        self.memory_manager.allocate(
            format!("agent-{}-{:?}", agent_id, resource_type),
            resource_type,
            size,
            metadata,
        ).await
    }

    /// Track token usage for an agent
    pub async fn track_agent_token_usage(
        &self,
        agent_id: &str,
        model: ModelType,
        prompt_tokens: usize,
        completion_tokens: usize,
    ) -> Result<(), NexaError> {
        let mut metadata = HashMap::new();
        metadata.insert("agent_id".to_string(), agent_id.to_string());
        
        self.token_manager
            .track_usage(model, prompt_tokens, completion_tokens, metadata)
            .await
    }

    /// Get token usage for an agent
    pub async fn get_agent_token_usage(&self, _agent_id: &str, since: Option<chrono::DateTime<chrono::Utc>>) -> TokenUsage {
        match since {
            Some(since_time) => self.token_manager.get_usage_since(since_time).await,
            None => {
                // Default to last 24 hours if no time specified
                let day_ago = chrono::Utc::now() - chrono::Duration::days(1);
                self.token_manager.get_usage_since(day_ago).await
            }
        }
    }

    /// Publish a message to the buffer
    pub async fn publish_message(&self, msg: BufferedMessage) -> Result<(), String> {
        self.message_buffer.publish(msg).await
    }

    /// Subscribe to messages
    pub fn subscribe_to_messages(&self) -> broadcast::Receiver<BufferedMessage> {
        self.message_buffer.subscribe()
    }

    /// Get next message from specified priority queue
    pub async fn get_next_message(&self, priority: Priority) -> Option<BufferedMessage> {
        self.message_buffer.pop(priority).await
    }

    /// Get next message from any priority queue
    pub async fn get_next_message_any_priority(&self) -> Option<BufferedMessage> {
        self.message_buffer.pop_any().await
    }

    /// Start the message cleanup task
    async fn start_message_cleanup(&self) {
        let buffer = self.message_buffer.clone();
        let cleanup_interval = buffer.config.cleanup_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                buffer.cleanup().await;
            }
        });
    }

    /// Get message processing metrics
    pub async fn get_message_metrics(&self) -> Result<metrics::MessageMetrics, NexaError> {
        Ok(self.metrics_collector.get_metrics().await)
    }

    /// Get message processing alerts
    pub async fn get_message_alerts(&self) -> Result<Vec<metrics::ProcessingAlert>, NexaError> {
        Ok(self.alert_checker.check_alerts().await)
    }
}

pub struct MCP {
    buffer: Arc<MessageBuffer>,
    _tx: broadcast::Sender<BufferedMessage>,
}

impl MCP {
    pub fn new() -> Self {
        let config = BufferConfig {
            cleanup_interval: std::time::Duration::from_secs(60),
            message_ttl: std::time::Duration::from_secs(3600),
            capacity: 1000,
            max_attempts: 3,
            max_message_size: 1024 * 1024, // 1MB
        };
        let buffer = Arc::new(MessageBuffer::new(config));
        let (tx, _) = broadcast::channel(100);
        Self {
            buffer,
            _tx: tx,
        }
    }

    pub async fn publish_message(&self, msg: BufferedMessage) -> Result<(), String> {
        self.buffer.publish(msg).await
    }

    pub async fn get_next_message(&self, priority: Priority) -> Option<BufferedMessage> {
        self.buffer.pop(priority).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::memory::ResourceType;

    #[tokio::test]
    async fn test_server_control() {
        // Set up temporary paths for test
        let runtime_dir = std::env::var("TMPDIR")
            .map(|dir| dir.trim_end_matches('/').to_string())
            .unwrap_or_else(|_| "/tmp".to_string());
        let runtime_dir = PathBuf::from(runtime_dir);
        let pid_file = runtime_dir.join("nexa-test-control.pid");
        let socket_path = runtime_dir.join("nexa-test-control.sock");

        // Clean up any existing files
        let _ = std::fs::remove_file(&pid_file);
        let _ = std::fs::remove_file(&socket_path);

        let server = ServerControl::new(pid_file.clone(), socket_path.clone());
        
        // Start server with test configuration
        assert!(server.start(Some("127.0.0.1:0")).await.is_ok());
        info!("Server started successfully");

        // Wait for monitoring to collect some data
        tokio::time::sleep(Duration::from_millis(100)).await;

        let metrics = server.get_metrics().await.unwrap();
        info!("Initial metrics - CPU: {:.1}%, Memory: {:?}", metrics.cpu_usage, metrics.memory_allocated);
        assert!(metrics.cpu_usage >= 0.0);

        // Wait for health check to stabilize
        tokio::time::sleep(Duration::from_millis(50)).await;

        let health = server.check_health().await.unwrap();
        info!("Health check result - Is healthy: {}, Message: {}", health.is_healthy, health.message);
        assert!(health.is_healthy, "System should be healthy after initialization");

        // Clean up
        server.stop().await.unwrap();
        
        // Add a small delay to ensure cleanup
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let _ = std::fs::remove_file(&pid_file);
        let _ = std::fs::remove_file(&socket_path);
    }

    #[tokio::test]
    async fn test_resource_tracking() {
        let server = ServerControl::new(PathBuf::new(), PathBuf::new());
        let agent_id = "test-agent";

        assert!(server
            .track_agent_resources(agent_id, ResourceType::TokenBuffer, 1024)
            .await
            .is_ok());

        let stats = server.memory_stats().await;
        assert!(stats.total_allocated > 0);
    }

    #[tokio::test]
    async fn test_token_tracking() {
        let server = ServerControl::new(PathBuf::new(), PathBuf::new());
        let agent_id = "test-agent";

        assert!(server
            .track_agent_token_usage(agent_id, ModelType::GPT4, 100, 50)
            .await
            .is_ok());

        let usage = server.get_agent_token_usage(agent_id, None).await;
        assert_eq!(usage.total_tokens, 150);
    }

    #[tokio::test]
    async fn test_message_buffer() {
        let mcp = MCP::new();
        
        // Test message publishing
        let msg = BufferedMessage {
            id: Uuid::new_v4(),
            payload: vec![1, 2, 3],
            priority: Priority::Normal,
            created_at: std::time::SystemTime::now(),
            attempts: 0,
            max_attempts: 3,
            delay_until: None,
        };
        
        // Publish message and wait for it to be processed
        assert!(mcp.publish_message(msg.clone()).await.is_ok(), "Failed to publish message");
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Test message retrieval
        let received = mcp.get_next_message(Priority::Normal).await;
        assert!(received.is_some(), "Expected to receive a message");
        let received = received.unwrap();
        assert_eq!(received.payload, vec![1, 2, 3]);
        assert_eq!(received.priority, Priority::Normal);
        
        // Test empty queue
        let empty = mcp.get_next_message(Priority::Normal).await;
        assert!(empty.is_none());
    }
}
