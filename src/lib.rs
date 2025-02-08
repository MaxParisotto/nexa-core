pub mod api;
pub mod cli;
pub mod error;
pub mod gui;
pub mod llm;
pub mod mcp;
pub mod memory;
pub mod models;
pub mod monitoring;
pub mod pipeline;
pub mod server;
pub mod tokens;
pub mod utils;
pub mod types;
pub mod settings;
pub mod logging;

// Re-export commonly used types
pub use models::agent::{Agent, Task, AgentStatus, TaskStatus};
pub use llm::system_helper::TaskPriority;

// Error type
pub use error::NexaError;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
