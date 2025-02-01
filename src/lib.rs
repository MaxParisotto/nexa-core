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
