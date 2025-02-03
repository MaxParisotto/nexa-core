pub mod api;
pub mod error;
pub mod monitoring;
pub mod cli;
pub mod server;
pub mod mcp;
pub mod agent;
pub mod agent_types;
pub mod config;
pub mod context;
pub mod llm;
pub mod memory;
pub mod pipeline;
pub mod tokens;
pub mod utils;

// Re-export common types
pub use error::NexaError;
pub use server::Server;
pub use config::Config;
pub use memory::MemoryManager;
pub use tokens::TokenManager;
pub use mcp::registry::AgentRegistry;
pub use mcp::protocol::ProtocolHandler;
pub use mcp::cluster::manager::ClusterManager;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
