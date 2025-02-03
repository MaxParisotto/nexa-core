//! Cluster Management Module
//! 
//! Provides distributed cluster management:
//! - Node discovery and coordination
//! - Leader election using Raft consensus
//! - State replication and synchronization
//! - Health monitoring and failure detection

mod types;
pub mod manager;

// Re-export commonly used types
pub use types::{
    Node, NodeRole, NodeHealth, NodeCapabilities,
    ClusterState, ClusterConfig, ClusterMessage,
    MembershipChange,
};
pub use manager::ClusterManager; 