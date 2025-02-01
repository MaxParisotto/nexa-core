use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use std::collections::{HashMap, VecDeque};
use tokio::net::TcpStream;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};
use tracing::{debug, error, warn};
use crate::error::NexaError;

#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub created_at: SystemTime,
    pub last_used: SystemTime,
    pub total_requests: u64,
    pub errors: u64,
    pub avg_response_time: Duration,
}

pub struct PooledConnection {
    pub stream: Arc<TcpStream>,
    pub created_at: SystemTime,
}

impl Clone for PooledConnection {
    fn clone(&self) -> Self {
        Self {
            stream: self.stream.clone(),
            created_at: self.created_at,
        }
    }
}

impl PooledConnection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream: Arc::new(stream),
            created_at: SystemTime::now(),
        }
    }

    pub async fn get_stream(&self) -> Result<TcpStream, NexaError> {
        // Try to create a new TcpStream from the existing connection
        let addr = self.stream.peer_addr()?;
        TcpStream::connect(addr).await.map_err(Into::into)
    }
}

pub struct ConnectionPool {
    available: VecDeque<PooledConnection>,
    in_use: HashMap<SocketAddr, PooledConnection>,
    max_size: usize,
    min_size: usize,
    connection_timeout: Duration,
    max_lifetime: Duration,
    semaphore: Arc<Semaphore>,
}

pub struct LoadBalancer {
    pools: Arc<RwLock<HashMap<SocketAddr, Arc<RwLock<ConnectionPool>>>>>,
    max_retries: usize,
    retry_delay: Duration,
    health_check_interval: Duration,
    connection_timeout: Duration,
}

impl ConnectionPool {
    pub fn new(
        max_size: usize,
        min_size: usize,
        connection_timeout: Duration,
        max_lifetime: Duration,
    ) -> Self {
        Self {
            available: VecDeque::with_capacity(max_size),
            in_use: HashMap::with_capacity(max_size),
            max_size,
            min_size,
            connection_timeout,
            max_lifetime,
            semaphore: Arc::new(Semaphore::new(max_size)),
        }
    }

    pub async fn acquire(&mut self, addr: SocketAddr) -> Result<TcpStream, NexaError> {
        // Try to get a permit from the semaphore
        let _permit = self.semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| NexaError::system(format!("Failed to acquire connection: {}", e)))?;

        // First try to get an available connection
        if let Some(conn) = self.available.pop_front() {
            // Check if connection is still valid
            if SystemTime::now()
                .duration_since(conn.created_at)
                .unwrap_or(Duration::from_secs(0)) > self.max_lifetime
            {
                // Connection is too old, create a new one
                debug!("Connection too old, creating new one");
                let new_conn = self.create_connection(addr).await?;
                self.in_use.insert(addr, new_conn.clone());
                return new_conn.get_stream().await;
            }
            
            self.in_use.insert(addr, conn.clone());
            return conn.get_stream().await;
        }

        // If no available connections and we haven't reached max_size, create a new one
        if self.in_use.len() < self.max_size {
            let conn = self.create_connection(addr).await?;
            self.in_use.insert(addr, conn.clone());
            return conn.get_stream().await;
        }

        Err(NexaError::system("Connection pool exhausted"))
    }

    pub async fn release(&mut self, addr: SocketAddr, stream: TcpStream) {
        if let Some(mut conn) = self.in_use.remove(&addr) {
            conn.stream = Arc::new(stream);
            conn.created_at = SystemTime::now();
            self.available.push_back(conn);
        }
    }

    async fn create_connection(&self, addr: SocketAddr) -> Result<PooledConnection, NexaError> {
        let stream = tokio::time::timeout(
            self.connection_timeout,
            TcpStream::connect(addr)
        ).await
        .map_err(|_| NexaError::system("Connection timeout"))??;

        Ok(PooledConnection {
            stream: Arc::new(stream),
            created_at: SystemTime::now(),
        })
    }

    pub async fn cleanup(&mut self) {
        let now = SystemTime::now();
        
        // Remove old connections from available pool
        self.available.retain(|conn| {
            now.duration_since(conn.created_at)
                .map(|age| age <= self.max_lifetime)
                .unwrap_or(false)
        });

        // Ensure minimum connections
        while self.available.len() < self.min_size {
            // TODO: Create new connections up to min_size
        }
    }
}

impl LoadBalancer {
    pub fn new(
        max_retries: usize,
        retry_delay: Duration,
        health_check_interval: Duration,
        connection_timeout: Duration,
    ) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            max_retries,
            retry_delay,
            health_check_interval,
            connection_timeout,
        }
    }

    pub async fn get_connection(&self, addr: SocketAddr) -> Result<TcpStream, NexaError> {
        let mut retries = 0;
        let mut last_error = None;

        while retries < self.max_retries {
            match self.try_get_connection(addr).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    last_error = Some(e);
                    retries += 1;
                    if retries < self.max_retries {
                        tokio::time::sleep(self.retry_delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| NexaError::system("Failed to get connection")))
    }

    pub async fn get_connection_for_server(&self, _server_id: &str, addr: SocketAddr) -> Result<TcpStream, NexaError> {
        self.get_connection(addr).await
    }

    async fn try_get_connection(&self, addr: SocketAddr) -> Result<TcpStream, NexaError> {
        let mut pools = self.pools.write().await;
        
        // Get or create pool for this address
        let pool = pools
            .entry(addr)
            .or_insert_with(|| {
                Arc::new(RwLock::new(ConnectionPool::new(
                    100, // max_size
                    10,  // min_size
                    self.connection_timeout,
                    Duration::from_secs(300), // max_lifetime
                )))
            })
            .clone();

        let mut pool = pool.write().await;
        pool.acquire(addr).await
    }

    pub async fn release_connection(&self, addr: SocketAddr, stream: TcpStream) {
        if let Some(pool) = self.pools.read().await.get(&addr) {
            let mut pool = pool.write().await;
            pool.release(addr, stream).await;
        }
    }

    pub async fn start_health_checks(&self) {
        let lb = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(lb.health_check_interval);
            loop {
                interval.tick().await;
                lb.check_pools_health().await;
            }
        });
    }

    async fn check_pools_health(&self) {
        let pools = self.pools.read().await;
        for (addr, pool) in pools.iter() {
            let mut pool = pool.write().await;
            if let Err(e) = self.check_pool_health(&mut pool, *addr).await {
                error!("Health check failed for {}: {}", addr, e);
            }
        }
    }

    async fn check_pool_health(&self, pool: &mut ConnectionPool, addr: SocketAddr) -> Result<(), NexaError> {
        // Cleanup old connections
        pool.cleanup().await;

        // Try to establish a test connection
        match pool.create_connection(addr).await {
            Ok(_) => {
                debug!("Health check passed for {}", addr);
                Ok(())
            }
            Err(e) => {
                warn!("Health check failed for {}: {}", addr, e);
                Err(e)
            }
        }
    }
}

impl Clone for LoadBalancer {
    fn clone(&self) -> Self {
        Self {
            pools: self.pools.clone(),
            max_retries: self.max_retries,
            retry_delay: self.retry_delay,
            health_check_interval: self.health_check_interval,
            connection_timeout: self.connection_timeout,
        }
    }
} 