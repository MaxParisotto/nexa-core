use nexa_core::mcp::cluster::ClusterConfig;
use std::time::Duration;
use nexa_core::error::NexaError;
use proptest::prelude::*;
use uuid::Uuid;

proptest! {
    #[test]
    fn test_cluster_config_generation(
        heartbeat_ms in 100u64..1000,
        election_ms in 1000u64..5000,
        quorum_size in 2usize..10,
        _node_count in 3usize..20,
    ) {
        let config = ClusterConfig {
            heartbeat_interval: Duration::from_millis(heartbeat_ms),
            election_timeout: (
                Duration::from_millis(election_ms),
                Duration::from_millis(election_ms * 2)
            ),
            min_quorum_size: quorum_size,
            node_timeout: Duration::from_secs(5),
            replication_factor: 3,
            cluster_id: Uuid::new_v4().to_string(),
        };

        // Validate configuration
        prop_assert!(config.election_timeout.0 > config.heartbeat_interval);
        prop_assert!(config.min_quorum_size >= 2);
    }
}

#[tokio::test]
async fn test_cluster_config() -> Result<(), NexaError> {
    // Initialize cluster configuration
    let config = ClusterConfig {
        heartbeat_interval: Duration::from_millis(100),
        election_timeout: (
            Duration::from_millis(500),
            Duration::from_millis(1000)
        ),
        min_quorum_size: 2,
        node_timeout: Duration::from_secs(5),
        replication_factor: 3,
        cluster_id: Uuid::new_v4().to_string(),
    };

    // Validate configuration
    assert!(config.election_timeout.0 > config.heartbeat_interval);
    assert!(config.min_quorum_size >= 2);

    Ok(())
} 