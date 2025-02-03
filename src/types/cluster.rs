//! Cluster Management Types
//! 
//! Core types for cluster management:
//! - Node identification and roles
//! - Cluster state and membership
//! - Health and status tracking

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

/// Node roles in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    /// Leader node coordinates cluster operations
    Leader,
    /// Follower nodes handle tasks and replicate state
    Follower,
    /// Candidate nodes participate in leader election
    Candidate,
    /// Observer nodes don't participate in voting
    Observer,
}

/// Node health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeHealth {
    /// Node is healthy and operating normally
    Healthy,
    /// Node is experiencing issues but still functional
    Degraded,
    /// Node is not functioning properly
    Unhealthy,
    /// Node status is unknown
    Unknown,
}

/// Node capabilities and resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Available CPU cores
    pub cpu_cores: u32,
    /// Total memory in MB
    pub memory_mb: u64,
    /// Supported task types
    pub task_types: Vec<String>,
    /// Custom capabilities
    pub custom: std::collections::HashMap<String, String>,
}

/// Node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique node identifier
    pub id: Uuid,
    /// Node network address
    pub addr: SocketAddr,
    /// Current node role
    pub role: NodeRole,
    /// Node health status
    pub health: NodeHealth,
    /// Node capabilities
    pub capabilities: NodeCapabilities,
    /// Last heartbeat timestamp
    pub last_heartbeat: SystemTime,
    /// Current term (for leader election)
    pub term: u64,
    /// Node labels for task scheduling
    pub labels: std::collections::HashMap<String, String>,
}

/// Cluster membership change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MembershipChange {
    /// Node joined the cluster
    Join {
        node: Node,
        timestamp: SystemTime,
    },
    /// Node left the cluster gracefully
    Leave {
        node_id: Uuid,
        timestamp: SystemTime,
    },
    /// Node was removed from cluster (failure/timeout)
    Remove {
        node_id: Uuid,
        reason: String,
        timestamp: SystemTime,
    },
}

/// Cluster state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterState {
    /// Current term number
    pub term: u64,
    /// Current leader ID
    pub leader_id: Option<Uuid>,
    /// Active nodes in the cluster
    pub nodes: std::collections::HashMap<Uuid, Node>,
    /// Minimum nodes for quorum
    pub quorum_size: usize,
    /// Cluster configuration version
    pub config_version: u64,
    /// Last state update timestamp
    pub last_updated: SystemTime,
}

/// Cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Election timeout range (min, max)
    pub election_timeout: (Duration, Duration),
    /// Minimum nodes for quorum
    pub min_quorum_size: usize,
    /// Node failure timeout
    pub node_timeout: Duration,
    /// State replication factor
    pub replication_factor: usize,
    /// Cluster name/ID
    pub cluster_id: String,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_millis(100),
            election_timeout: (Duration::from_millis(150), Duration::from_millis(300)),
            min_quorum_size: 3,
            node_timeout: Duration::from_secs(5),
            replication_factor: 3,
            cluster_id: "nexa-cluster".to_string(),
        }
    }
}

/// Message types for cluster communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterMessage {
    RequestVote {
        term: u64,
        candidate_id: Uuid,
    },
    VoteResponse {
        term: u64,
        voter_id: Uuid,
        granted: bool,
    },
    Heartbeat {
        term: u64,
        leader_id: Uuid,
        timestamp: SystemTime,
    },
    MembershipChange(MembershipChange),
    StateSync {
        term: u64,
        state: ClusterState,
    },
} 