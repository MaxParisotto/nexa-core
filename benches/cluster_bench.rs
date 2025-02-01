use criterion::{criterion_group, criterion_main, Criterion};
use nexa_core::mcp::{cluster::*, config::*};
use tokio::runtime::Runtime;
use std::time::Duration;

fn cluster_coordination_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cluster_coordination");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("node_election", |b| {
        b.iter(|| {
            rt.block_on(async {
                let coordinator = ClusterCoordinator::new(
                    "test-node".to_string(),
                    vec!["peer1".to_string(), "peer2".to_string()]
                );
                coordinator.start().await.unwrap();
                
                // Simulate election process
                coordinator.handle_event(ClusterEvent {
                    event_type: "vote_request".to_string(),
                    node_id: "test-node".to_string(),
                    timestamp: std::time::SystemTime::now(),
                    data: serde_json::json!({
                        "term": 1,
                        "health_score": 1.0,
                    }),
                }).await.unwrap();
            });
        })
    });

    group.bench_function("heartbeat_broadcast", |b| {
        b.iter(|| {
            rt.block_on(async {
                let coordinator = ClusterCoordinator::new(
                    "test-node".to_string(),
                    vec!["peer1".to_string(), "peer2".to_string()]
                );
                coordinator.start().await.unwrap();
                
                // Simulate heartbeat
                coordinator.handle_event(ClusterEvent {
                    event_type: "heartbeat".to_string(),
                    node_id: "leader-node".to_string(),
                    timestamp: std::time::SystemTime::now(),
                    data: serde_json::json!({
                        "term": 1,
                        "health_score": 1.0,
                    }),
                }).await.unwrap();
            });
        })
    });

    group.finish();
}

fn cluster_health_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cluster_health");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("health_check", |b| {
        b.iter(|| {
            rt.block_on(async {
                let coordinator = ClusterCoordinator::new(
                    "test-node".to_string(),
                    vec!["peer1".to_string(), "peer2".to_string()]
                );
                coordinator.start().await.unwrap();
                
                // Simulate health check
                coordinator.monitor_cluster_health().await.unwrap();
            });
        })
    });

    group.finish();
}

criterion_group!(benches, cluster_coordination_benchmark, cluster_health_benchmark);
criterion_main!(benches); 