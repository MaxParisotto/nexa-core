use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nexa_core::mcp::loadbalancer::*;
use tokio::runtime::Runtime;
use std::time::Duration;
use std::net::SocketAddr;
use std::str::FromStr;
use futures::future::join_all;

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
                let mut futures = Vec::new();
                for _ in 0..50 {
                    let lb = lb.clone();
                    futures.push(tokio::spawn(async move {
                        if let Ok(conn) = lb.get_connection(addr).await {
                            lb.release_connection(addr, conn).await;
                        }
                    }));
                }

                join_all(futures).await;
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

                // Start health checks and let them run for a short duration
                lb.start_health_checks().await;
                tokio::time::sleep(Duration::from_millis(100)).await;
            });
        })
    });

    group.finish();
}

pub fn loadbalancer_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("loadbalancer");
    let rt = Runtime::new().unwrap();

    group.bench_function("message_distribution", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Your benchmark code here
                // Removed unused handles variable
            });
        })
    });

    group.finish();
}

fn loadbalancer_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("loadbalancer_basic_ops", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Simulate basic load balancer operations
                black_box(async {
                    tokio::time::sleep(tokio::time::Duration::from_micros(1)).await;
                    Ok::<_, anyhow::Error>(())
                })
                .await
            })
        })
    });
}

criterion_group!(benches, connection_pool_benchmark, load_balancer_benchmark, loadbalancer_benchmark, loadbalancer_operations);
criterion_main!(benches); 