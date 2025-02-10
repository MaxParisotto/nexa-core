pub mod api;
pub mod cli;
pub mod config;
pub mod error;
pub mod llm;
pub mod mcp;
pub mod memory;
pub mod models;
pub mod monitoring;
pub mod server;
pub mod settings;
pub mod tokens;
pub mod types;
pub mod utils;

// Re-export commonly used types
pub use models::agent::{Agent, Task, ModelAgentStatus as AgentStatus, TaskStatus};
pub use llm::system_helper::TaskPriority;
pub use crate::config::{MonitoringConfig, ServerConfig, LoggingConfig};

// Error type
pub use error::NexaError;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
