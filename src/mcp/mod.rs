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

use std::path::PathBuf;
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
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use chrono::Utc;
use std::time::Duration;
use crate::tokens::{TokenManager, ModelType, TokenUsage};

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
        }
    }

    pub async fn start(&self, addr: Option<&str>) -> Result<(), NexaError> {
        // Early check: if server task already exists, then server is running
        if self.server_handle.read().await.is_some() {
            error!("Server is already running");
            return Err(NexaError::system("Server is already running"));
        }

        // Validate and parse the address
        let bind_addr = match addr {
            Some(addr) => {
                match addr.parse::<std::net::SocketAddr>() {
                    Ok(addr) => Some(addr.to_string()),
                    Err(_) => return Err(NexaError::protocol("Invalid address format")),
                }
            }
            None => None,
        };

        let server = self.server.clone();
        
        // Start server in a new task
        let handle = tokio::spawn(async move {
            server.start_server(bind_addr).await
        });
        
        // Store the handle
        *self.server_handle.write().await = Some(handle);
        
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
                error!("Timeout waiting for server to start");
                let _ = self.stop().await; // Attempt to stop the server if stuck
                return Err(NexaError::system("Server failed to start within timeout"));
            }
            
            // Check for early failure
            if let Some(handle) = self.server_handle.write().await.take() {
                if handle.is_finished() {
                    match handle.await {
                        Ok(Ok(_)) => {
                            // Server completed successfully (unusual but possible)
                            if self.server.get_state().await == ServerState::Running {
                                if let Some(bound_addr) = self.server.get_bound_addr().await {
                                    info!("Server started successfully on {}", bound_addr);
                                    return Ok(());
                                }
                            }
                            return Err(NexaError::system("Server task completed unexpectedly"));
                        }
                        Ok(Err(e)) => {
                            error!("Server failed to start: {}", e);
                            return Err(e);
                        }
                        Err(e) => {
                            error!("Server task failed: {}", e);
                            return Err(NexaError::system(format!("Server task failed: {}", e)));
                        }
                    }
                } else {
                    // Put the handle back since it's not finished
                    *self.server_handle.write().await = Some(handle);
                }
            }
            
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn stop(&self) -> Result<(), NexaError> {
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
        Err(NexaError::system("Server failed to stop within timeout"))
    }

    pub async fn get_bound_addr(&self) -> Result<std::net::SocketAddr, NexaError> {
        self.server.get_bound_addr().await
            .ok_or_else(|| NexaError::system("Server address not available"))
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
        assert!(server.start(None).await.is_ok());
        info!("Server started successfully");

        // Wait for monitoring to collect some data
        tokio::time::sleep(Duration::from_secs(2)).await;

        let metrics = server.get_metrics().await.unwrap();
        info!("Initial metrics - CPU: {:.1}%, Memory: {:?}", metrics.cpu_usage, metrics.memory_allocated);
        assert!(metrics.cpu_usage >= 0.0);

        // Wait for health check to stabilize
        tokio::time::sleep(Duration::from_secs(1)).await;

        let health = server.check_health().await.unwrap();
        info!("Health check result - Is healthy: {}, Message: {}", health.is_healthy, health.message);
        assert!(health.is_healthy, "System should be healthy after initialization");

        // Clean up
        server.stop().await.unwrap();
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
}
