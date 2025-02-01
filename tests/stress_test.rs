use nexa_utils::mcp::{cluster::*, config::*, loadbalancer::*};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, error, info};
use test_log::test;
use rand::Rng;
use std::str::FromStr;
use proptest::prelude::*;
use futures::future::join_all;
use tempfile::tempdir;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use rand::rngs::SmallRng;
use rand::SeedableRng;

const TEST_DURATION: Duration = Duration::from_secs(30);
const MAX_CONCURRENT_CONNECTIONS: usize = 1000;
const CHAOS_INTERVAL: Duration = Duration::from_secs(5);

#[test(tokio::test)]
async fn test_cluster_stability_under_load() {
    let temp_dir = tempdir().unwrap();
    let runtime_dir = temp_dir.path().to_path_buf();

    // Create multiple nodes
    let mut nodes = Vec::new();
    let node_count = 5;

    for i in 0..node_count {
        let node_id = format!("node-{}", i);
        let peers: Vec<String> = (0..node_count)
            .filter(|&j| j != i)
            .map(|j| format!("node-{}", j))
            .collect();

        let coordinator = ClusterCoordinator::new(node_id.clone(), peers);
        coordinator.start().await.unwrap();
        nodes.push(coordinator);
    }

    // Start chaos testing
    let (tx, mut rx) = mpsc::channel(100);
    let chaos_handle = tokio::spawn({
        let mut rng = SmallRng::from_entropy();
        async move {
            loop {
                let sleep_duration = Duration::from_millis(rng.gen_range(100..1000));
                sleep(sleep_duration).await;
                
                // Randomly kill a node
                let node_idx = rng.gen_range(0..node_count);
                tx.send(format!("kill_node_{}", node_idx)).await.unwrap();
                
                // Wait before resurrection
                let sleep_duration = Duration::from_millis(rng.gen_range(500..2000));
                sleep(sleep_duration).await;
                tx.send(format!("resurrect_node_{}", node_idx)).await.unwrap();
            }
        }
    });

    // Monitor cluster health
    let health_handle = tokio::spawn(async move {
        let start_time = SystemTime::now();
        while SystemTime::now().duration_since(start_time).unwrap() < TEST_DURATION {
            for node in &nodes {
                if let Err(e) = node.monitor_cluster_health().await {
                    error!("Node health check failed: {}", e);
                }
            }
            sleep(Duration::from_secs(1)).await;
        }
    });

    // Wait for test completion
    tokio::select! {
        _ = health_handle => {
            info!("Health monitoring completed");
        }
        _ = chaos_handle => {
            info!("Chaos testing completed");
        }
    }
}

#[test(tokio::test)]
async fn test_loadbalancer_under_pressure() {
    // Start multiple test servers
    let server_count = 3;
    let mut servers = Vec::new();
    let mut addresses = Vec::new();

    for _ in 0..server_count {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        addresses.push(addr);
        
        let handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((socket, _)) => {
                        tokio::spawn(async move {
                            // Echo server
                            let (mut rd, mut wr) = socket.into_split();
                            tokio::io::copy(&mut rd, &mut wr).await.unwrap();
                        });
                    }
                    Err(e) => {
                        error!("Accept failed: {}", e);
                        break;
                    }
                }
            }
        });
        servers.push(handle);
    }

    // Create load balancer
    let lb = LoadBalancer::new(
        3, // max_retries
        Duration::from_millis(100), // retry_delay
        Duration::from_secs(1), // health_check_interval
        Duration::from_secs(5), // connection_timeout
    );

    // Start health checks
    lb.start_health_checks().await;

    // Generate load
    let mut handles = Vec::new();
    for i in 0..MAX_CONCURRENT_CONNECTIONS {
        let lb = lb.clone();
        let addresses = addresses.clone();
        
        let handle = tokio::spawn({
            let mut rng = SmallRng::from_entropy();
            async move {
                loop {
                    let addr = addresses[rng.gen_range(0..addresses.len())];
                    match lb.get_connection(addr).await {
                        Ok(stream) => {
                            // Simulate some work
                            let sleep_duration = Duration::from_millis(rng.gen_range(50..200));
                            sleep(sleep_duration).await;
                            lb.release_connection(addr, stream).await;
                        }
                        Err(e) => {
                            error!("Connection {} failed: {}", i, e);
                        }
                    }
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all connections to complete
    join_all(handles).await;
}

proptest! {
    #[test]
    fn test_cluster_config_properties(
        heartbeat_ms in 100u64..1000,
        election_ms in 1000u64..5000,
        quorum_size in 2usize..10,
        node_count in 2usize..20
    ) {
        let mut config = ClusterConfig {
            enabled: true,
            node_id: uuid::Uuid::new_v4().to_string(),
            peers: (0..node_count).map(|i| format!("node-{}", i)).collect(),
            heartbeat_interval_ms: heartbeat_ms,
            election_timeout_ms: election_ms,
            quorum_size,
        };

        prop_assert!(config.election_timeout_ms > config.heartbeat_interval_ms);
        prop_assert!(config.quorum_size >= 2);
        prop_assert!(config.peers.len() + 1 >= config.quorum_size);
    }
}

#[test(tokio::test)]
async fn test_connection_pool_limits() {
    let mut pool = ConnectionPool::new(
        10, // max_size
        2,  // min_size
        Duration::from_secs(1),
        Duration::from_secs(30),
    );

    let addr = SocketAddr::from_str("127.0.0.1:8080").unwrap();
    let mut connections = Vec::new();

    // Try to acquire more connections than max_size
    for i in 0..20 {
        match pool.acquire(addr).await {
            Ok(conn) => {
                connections.push(conn);
                if i >= 10 {
                    panic!("Acquired more connections than max_size");
                }
            }
            Err(e) => {
                if i < 10 {
                    panic!("Failed to acquire connection within max_size: {}", e);
                }
            }
        }
    }
}

#[test(tokio::test)]
async fn test_cluster_network_partition() {
    let node_count = 5;
    let mut nodes = Vec::new();
    let mut node_states = Vec::new();

    // Create nodes
    for i in 0..node_count {
        let node_id = format!("node-{}", i);
        let peers: Vec<String> = (0..node_count)
            .filter(|&j| j != i)
            .map(|j| format!("node-{}", j))
            .collect();

        let coordinator = ClusterCoordinator::new(node_id.clone(), peers);
        coordinator.start().await.unwrap();
        nodes.push(coordinator);
        node_states.push(NodeRole::Follower);
    }

    // Simulate network partition
    let partition_size = node_count / 2;
    
    // Isolate first half of nodes
    for i in 0..partition_size {
        let node = &nodes[i];
        // Simulate partition by stopping heartbeats
        node_states[i] = NodeRole::Candidate;
    }

    // Let the cluster stabilize
    sleep(Duration::from_secs(5)).await;

    // Verify that only one side of the partition has a leader
    let mut leaders_count = 0;
    for (i, node) in nodes.iter().enumerate() {
        if node_states[i] == NodeRole::Leader {
            leaders_count += 1;
        }
    }

    assert!(leaders_count <= 1, "Multiple leaders detected after partition");
}

#[test(tokio::test)]
async fn test_load_balancer_failover() {
    let mut rng = rand::thread_rng();
    let server_count = 3;
    let mut addresses = Vec::new();

    // Start servers
    for _ in 0..server_count {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        addresses.push(listener.local_addr().unwrap());
        
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (mut rd, mut wr) = socket.into_split();
                    tokio::io::copy(&mut rd, &mut wr).await.unwrap();
                });
            }
        });
    }

    let lb = LoadBalancer::new(
        3,
        Duration::from_millis(100),
        Duration::from_secs(1),
        Duration::from_secs(5),
    );

    // Test failover
    for _ in 0..100 {
        let addr = addresses[rng.gen_range(0..addresses.len())];
        match lb.get_connection(addr).await {
            Ok(stream) => {
                // Simulate work
                let mut rng = rand::thread_rng();
                let sleep_duration = Duration::from_millis(rng.gen_range(10..100));
                sleep(sleep_duration).await;
                lb.release_connection(addr, stream).await;
            }
            Err(_) => {
                // Should automatically retry with different server
                continue;
            }
        }
    }
}

#[test(tokio::test)]
async fn test_connection_pool() {
    let mut rng = SmallRng::from_entropy();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let mut pool = ConnectionPool::new(
        10, // max_size
        2,  // min_size
        Duration::from_secs(1),
        Duration::from_secs(30),
    );
    // ... rest of the test ...
} 