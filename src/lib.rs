pub mod error;
pub mod utils;
pub mod agent;
pub mod mcp;
pub mod agent_types;
pub mod memory;
pub mod tokens;
pub mod monitoring;
pub mod api;
pub mod cli;

// Re-export commonly used types
pub use agent::{Agent, AgentStatus, Task, TaskStatus};
pub use error::{NexaError, Result};
pub use memory::{MemoryManager, MemoryStats, ResourceType};
pub use tokens::{TokenManager, TokenUsage, ModelType};
pub use monitoring::{
    MonitoringSystem, SystemMetrics, SystemAlert, AlertLevel,
    SystemHealth, SystemStatus
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum NexaError {
        #[error("Protocol error: {0}")]
        Protocol(String),
        
        #[error("Agent error: {0}")]
        Agent(String),
        
        #[error("System error: {0}")]
        System(String),
        
        #[error("Configuration error: {0}")]
        ConfigError(String),
        
        #[error("WebSocket error: {0}")]
        WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
        
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),
        
        #[error("YAML error: {0}")]
        Yaml(#[from] serde_yaml::Error),
        
        #[error("JSON error: {0}")]
        Json(#[from] serde_json::Error),
    }

    impl NexaError {
        pub fn protocol(msg: impl Into<String>) -> Self {
            NexaError::Protocol(msg.into())
        }

        pub fn agent(msg: impl Into<String>) -> Self {
            NexaError::Agent(msg.into())
        }

        pub fn system(msg: impl Into<String>) -> Self {
            NexaError::System(msg.into())
        }

        pub fn config(msg: impl Into<String>) -> Self {
            NexaError::ConfigError(msg.into())
        }
    }

    pub type Result<T> = std::result::Result<T, NexaError>;
}

pub use error::{NexaError, Result};

pub mod api;
pub mod cli;
pub mod mcp;
pub mod monitoring;
