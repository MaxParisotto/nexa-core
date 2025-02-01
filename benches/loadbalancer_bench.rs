use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nexa_utils::mcp::loadbalancer::*;
use tokio::runtime::Runtime;
use std::time::Duration;
use std::net::SocketAddr;
use std::str::FromStr;

fn connection_pool_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let addr = SocketAddr::from_str("127.0.0.1:8080").unwrap();

    let mut group = c.benchmark_group("connection_pool");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("acquire_release", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut pool = ConnectionPool::new(
                    100,
                    10,
                    Duration::from_secs(1),
                    Duration::from_secs(30),
                );

                // Acquire and release multiple connections
                let mut handles = Vec::new();
                for _ in 0..10 {
                    if let Ok(conn) = pool.acquire(addr).await {
                        pool.release(addr, conn).await;
                    }
                }
            });
        })
    });

    group.bench_function("concurrent_connections", |b| {
        b.iter(|| {
            rt.block_on(async {
                let lb = LoadBalancer::new(
                    3,
                    Duration::from_millis(100),
                    Duration::from_secs(1),
                    Duration::from_secs(5),
                );

                // Simulate concurrent connection requests
                let mut handles = Vec::new();
                for _ in 0..50 {
                    let lb = lb.clone();
                    handles.push(tokio::spawn(async move {
                        if let Ok(conn) = lb.get_connection(addr).await {
                            lb.release_connection(addr, conn).await;
                        }
                    }));
                }

                futures::future::join_all(handles).await;
            });
        })
    });

    group.finish();
}

fn load_balancer_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let addr = SocketAddr::from_str("127.0.0.1:8080").unwrap();

    let mut group = c.benchmark_group("load_balancer");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("connection_management", |b| {
        b.iter(|| {
            rt.block_on(async {
                let lb = LoadBalancer::new(
                    3,
                    Duration::from_millis(100),
                    Duration::from_secs(1),
                    Duration::from_secs(5),
                );

                // Test connection acquisition with retries
                for _ in 0..10 {
                    if let Ok(conn) = lb.get_connection(addr).await {
                        lb.release_connection(addr, conn).await;
                    }
                }
            });
        })
    });

    group.bench_function("health_checks", |b| {
        b.iter(|| {
            rt.block_on(async {
                let lb = LoadBalancer::new(
                    3,
                    Duration::from_millis(100),
                    Duration::from_secs(1),
                    Duration::from_secs(5),
                );

                let pools = lb.pools.write().await;
                for (addr, pool) in pools.iter() {
                    let mut pool = pool.write().await;
                    let _ = lb.check_pool_health(&mut pool, *addr).await;
                }
            });
        })
    });

    group.finish();
}

criterion_group!(benches, connection_pool_benchmark, load_balancer_benchmark);
criterion_main!(benches); 