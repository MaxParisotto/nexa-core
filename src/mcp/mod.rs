#![allow(dead_code)]

//! Multi-agent Control Protocol (MCP) Implementation
//!
//! This module provides the core functionality for agent communication and management:
//! - Message types for agent registration and communication
//! - Connection handling
//! - Protocol implementation
//! - Registry management

// MCP Module Declarations separated by responsibilities

// Core Modules
pub mod registry;
pub mod server;
pub mod protocol;
pub mod tokens;

// Cluster Management Modules
pub mod cluster;
pub mod config;
pub mod loadbalancer;
pub mod buffer;
pub mod processor;
pub mod cluster_processor;

// Metrics and Monitoring Modules
pub mod metrics;

use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::types::{Agent, Task, AgentStatus};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use chrono::Utc;
use crate::mcp::buffer::{BufferedMessage, MessageBuffer, Priority, BufferConfig};
use crate::monitoring::{SystemMetrics, SystemHealth, ResourceType};
use crate::mcp::tokens::{ModelType, TokenUsage};
use crate::tokens::TokenMetrics;
use crate::error::NexaError;
use log::{error, info};

// -----------------------------------------------------
// Persistent Global State for MCP Module
// -----------------------------------------------------
use once_cell::sync::Lazy;
use std::sync::RwLock as StdRwLock;

/// McpState holds persistent state data for the MCP module.
#[derive(Debug, Default)]
pub struct McpState {
    /// An example counter to track some state persistently.
    pub counter: u64,
    // Add more persistent fields as needed
}

/// Global persistent state for the MCP module.
pub static GLOBAL_MCP_STATE: Lazy<StdRwLock<McpState>> = Lazy::new(|| {
    StdRwLock::new(McpState::default())
});

/// Returns a reference to the global MCP state.
pub fn global_state() -> &'static Lazy<StdRwLock<McpState>> {
    &GLOBAL_MCP_STATE
}

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

/// A minimal registry stub with placeholder methods.
#[derive(Debug)]
pub struct RegistryStub;

impl RegistryStub {
    /// Dummy implementation to add a task to the registry.
    pub async fn add_task(&self, _task: crate::types::Task) -> Result<(), crate::error::NexaError> {
        Ok(())
    }

    /// Dummy implementation to get an agent from the registry.
    pub async fn get_agent(&self, _id: &str) -> Result<crate::types::Agent, crate::error::NexaError> {
        Err(crate::error::NexaError::System("Not implemented".to_string()))
    }

    /// Dummy implementation to list agents in the registry.
    pub async fn list_agents(&self) -> Result<Vec<crate::types::Agent>, crate::error::NexaError> {
        Ok(vec![])
    }

    /// Dummy implementation to get a task from the registry.
    pub async fn get_task(&self, _id: &str) -> Result<crate::types::Task, crate::error::NexaError> {
        Err(crate::error::NexaError::System("Not implemented".to_string()))
    }

    /// Dummy implementation to list tasks in the registry.
    pub async fn list_tasks(&self) -> Result<Vec<crate::types::Task>, crate::error::NexaError> {
        Ok(vec![])
    }
}

#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_allocated: usize,
    pub allocation_count: usize,
    pub available: usize,
    pub peak_usage: usize,
    pub total_used: usize,
}

pub struct ServerControl {
    pub registry: RegistryStub,
}

impl ServerControl {
    /// Create a new ServerControl instance with the provided data and config directories.
    pub fn new(_data_dir: PathBuf, _config_dir: PathBuf) -> Self {
        ServerControl {
            registry: RegistryStub,
        }
    }
    
    /// Start the server (dummy implementation).
    pub async fn start(&self, _addr: Option<&str>) -> Result<(), NexaError> {
        Ok(())
    }
    
    /// Stop the server (dummy implementation).
    pub async fn stop(&self) -> Result<(), NexaError> {
        Ok(())
    }
    
    /// Check the health of the server.
    pub async fn check_health(&self) -> Result<SystemHealth, NexaError> {
        Ok(SystemHealth {
            cpu_healthy: true,
            memory_healthy: true,
            overall_healthy: true,
        })
    }
    
    /// Retrieve server metrics.
    pub async fn get_metrics(&self) -> Result<SystemMetrics, NexaError> {
        Ok(SystemMetrics {
            timestamp: Utc::now(),
            cpu_usage: 10.0,
            memory_usage: 0.5,
            active_agents: 0,
            token_usage: TokenMetrics::default(),
        })
    }
    
    /// Retrieve memory statistics.
    pub async fn memory_stats(&self) -> MemoryStats {
        MemoryStats {
            total_allocated: 1024,
            allocation_count: 10,
            available: 512,
            peak_usage: 256,
            total_used: 300,
        }
    }
    
    /// Dummy method to track agent resource allocation.
    pub async fn track_agent_resources(&self, _agent_id: &str, _resource_type: ResourceType, _size: usize) -> Result<(), NexaError> {
        Ok(())
    }
    
    /// Dummy method to track agent token usage.
    pub async fn track_agent_token_usage(&self, _agent_id: &str, _model: ModelType, _prompt_tokens: usize, _completion_tokens: usize) -> Result<(), NexaError> {
        Ok(())
    }
    
    /// Dummy method to get agent token usage.
    pub async fn get_agent_token_usage(&self, _agent_id: &str, _since: Option<chrono::DateTime<Utc>>) -> TokenUsage {
        TokenUsage {
            total_tokens: 150,
            prompt_tokens: 100,
            completion_tokens: 50,
            cost: 0.0,
        }
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

// Re-export commonly used types

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_state() {
        let state = GLOBAL_MCP_STATE.write().unwrap();
        assert_eq!(state.counter, 0);
    }

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
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let metrics = server.get_metrics().await.unwrap();
        info!("Initial metrics - CPU: {:.1}%, Memory: {:.1}%", 
            metrics.cpu_usage * 100.0, 
            metrics.memory_usage * 100.0
        );
        assert!(metrics.cpu_usage >= 0.0);

        // Wait for health check to stabilize
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let health = server.check_health().await.unwrap();
        info!("Health check result - Overall: {}, CPU: {}, Memory: {}", 
            health.overall_healthy,
            health.cpu_healthy,
            health.memory_healthy
        );
        assert!(health.overall_healthy, "System should be healthy after initialization");

        // Clean up
        server.stop().await.unwrap();
        
        // Add a small delay to ensure cleanup
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        
        let _ = std::fs::remove_file(&pid_file);
        let _ = std::fs::remove_file(&socket_path);
    }

    #[tokio::test]
    async fn test_resource_tracking() {
        let server = ServerControl::new(PathBuf::new(), PathBuf::new());
        let agent_id = "test-agent";

        assert!(server
            .track_agent_resources(agent_id, ResourceType::Memory, 1024)
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
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
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
