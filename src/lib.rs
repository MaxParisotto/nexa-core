pub mod error;
pub use error::{NexaError, Result};

pub mod api;
pub mod cli;
pub mod mcp;
pub mod monitoring;
pub mod agent;
pub mod agent_types;
pub mod memory;
pub mod tokens;
pub mod utils;
pub mod config;
pub mod llm;

// Re-export commonly used types
pub use agent::{Agent, AgentStatus, Task, TaskStatus};
pub use memory::{MemoryManager, MemoryStats, ResourceType};
pub use tokens::{TokenManager, TokenUsage, ModelType};
pub use monitoring::{
    MonitoringSystem, SystemMetrics, SystemAlert, AlertLevel,
    SystemHealth, SystemStatus
};
pub use config::Config;
pub use mcp::ServerControl;
pub use llm::{LLMClient, LLMConfig};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
