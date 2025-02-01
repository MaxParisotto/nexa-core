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

criterion_group!(benches, cluster_benchmark);
criterion_main!(benches); 