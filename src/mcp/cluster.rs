use std::time::Duration;

/// ClusterConfig holds the configuration parameters for the cluster.
/// It includes heartbeat interval, election timeout, minimum quorum size, node timeout, replication factor, and a unique cluster identifier.
pub struct ClusterConfig {
    /// The interval for heartbeat signals.
    pub heartbeat_interval: Duration,
    /// The election timeout wrapped in ElectionTimeout newtype for added type safety.
    pub election_timeout: ElectionTimeout,
    /// The minimum number of nodes required to form a quorum.
    pub min_quorum_size: usize,
    /// The duration after which a node is considered unresponsive.
    pub node_timeout: Duration,
    /// The replication factor defining how many copies of data exist.
    pub replication_factor: usize,
    /// A unique identifier for the cluster.
    pub cluster_id: String,
}

/// ElectionTimeout is a newtype struct wrapping a tuple of Durations to represent the election timeout range.
pub struct ElectionTimeout(pub Duration, pub Duration);

pub mod types;
pub mod manager;
pub use manager::ClusterManager; 