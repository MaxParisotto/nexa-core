// Core modules
pub mod api;
pub mod cli;
pub mod config;
pub mod error;
pub mod llm;
pub mod mcp;
pub mod memory;
pub mod monitoring;
pub mod server;
pub mod startup;
pub mod tokens;
pub mod types;
pub mod utils;

// Re-export commonly used types and modules
pub use api::ApiServer;
pub use cli::CliHandler;
pub use error::NexaError;
pub use server::{Server, ServerState};
pub use config::{ServerConfig, MonitoringConfig, LoggingConfig, LLMConfig};
pub use types::{agent, workflow};
pub use memory::MemoryManager;
pub use tokens::TokenManager;
pub use startup::{
    StartupManager,
    CheckStatus,
};

// Monitoring system exports
#[cfg(feature = "monitoring")]
pub use monitoring::{
    MonitoringSystem,
    SystemMetrics,
    SystemHealth,
    SystemAlert,
    AlertLevel,
    ResourceMetrics,
};

#[cfg(test)]
mod tests {
}
