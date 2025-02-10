use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nexa_core::mcp::cluster::ClusterManager;
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::runtime::Runtime;

pub fn cluster_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("cluster");
    let rt = Runtime::new().unwrap();

    group.bench_function("election", |b| {
        b.iter(|| {
            rt.block_on(async {
                let addr = SocketAddr::from_str("127.0.0.1:8080").unwrap();
                let manager = ClusterManager::new(addr, None);
                
                // Simulate election
                black_box(manager.start_election().await.unwrap());
            });
        })
    });

    group.finish();
}

fn cluster_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("cluster_basic_ops", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Simulate basic cluster operations
                black_box(async {
                    tokio::time::sleep(tokio::time::Duration::from_micros(1)).await;
                    Ok::<_, anyhow::Error>(())
                })
                .await
            })
        })
    });
}

criterion_group!(benches, cluster_benchmark, cluster_operations);
criterion_main!(benches); 