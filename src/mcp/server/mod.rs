mod config;

pub use config::ServerConfig;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{RwLock, watch, Notify};
use tokio::net::{TcpListener, TcpStream};
use std::net::SocketAddr;
use log::{error, info};
use tokio_tungstenite::{WebSocketStream, tungstenite::protocol::Message};
use futures::stream::{SplitStream, SplitSink};
use futures::StreamExt;
use crate::error::NexaError;
use serde_json;

#[derive(Debug, Clone, PartialEq)]
pub enum ServerState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error(String),
    Maintenance,
}

impl std::str::FromStr for ServerState {
    type Err = NexaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stopped" => Ok(ServerState::Stopped),
            "starting" => Ok(ServerState::Starting),
            "running" => Ok(ServerState::Running),
            "stopping" => Ok(ServerState::Stopping),
            "maintenance" => Ok(ServerState::Maintenance),
            s if s.starts_with("error:") => Ok(ServerState::Error(s[6..].trim().to_string())),
            _ => Err(NexaError::Server(format!("Invalid server state: {}", s))),
        }
    }
}

impl std::fmt::Display for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerState::Stopped => write!(f, "stopped"),
            ServerState::Starting => write!(f, "starting"),
            ServerState::Running => write!(f, "running"),
            ServerState::Stopping => write!(f, "stopping"),
            ServerState::Error(msg) => write!(f, "error: {}", msg),
            ServerState::Maintenance => write!(f, "maintenance"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerMetrics {
    pub start_time: SystemTime,
    pub total_connections: u64,
    pub active_connections: u32,
    pub failed_connections: u64,
    pub last_error: Option<String>,
    pub uptime: Duration,
}

#[derive(Clone, Debug)]
pub struct Server {
    pid_file: PathBuf,
    #[allow(dead_code)]
    socket_path: PathBuf,
    bound_addr: Arc<RwLock<Option<std::net::SocketAddr>>>,
    state: Arc<RwLock<ServerState>>,
    shutdown_tx: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::Sender<()>>>>,
    active_connections: Arc<RwLock<u32>>,
    server_handle: Arc<RwLock<Option<tokio::task::JoinHandle<Result<(), NexaError>>>>>,
    ready_notify: Arc<Notify>,
    metrics: Arc<RwLock<ServerMetrics>>,
    health_check_interval: Duration,
    max_connections: u32,
    connection_timeout: Duration,
    #[allow(dead_code)]
    state_change_tx: Arc<watch::Sender<ServerState>>,
    #[allow(dead_code)]
    state_change_rx: watch::Receiver<ServerState>,
    connected_clients: Arc<RwLock<HashMap<SocketAddr, SystemTime>>>,
    config: Arc<RwLock<ServerConfig>>,
}

impl Server {
    pub fn new(pid_file: PathBuf, socket_path: PathBuf) -> Self {
        let (state_tx, state_rx) = watch::channel(ServerState::Stopped);
        
        Self {
            pid_file,
            socket_path,
            bound_addr: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(ServerState::Stopped)),
            shutdown_tx: Arc::new(tokio::sync::Mutex::new(None)),
            active_connections: Arc::new(RwLock::new(0)),
            server_handle: Arc::new(RwLock::new(None)),
            ready_notify: Arc::new(Notify::new()),
            metrics: Arc::new(RwLock::new(ServerMetrics {
                start_time: SystemTime::now(),
                total_connections: 0,
                active_connections: 0,
                failed_connections: 0,
                last_error: None,
                uptime: Duration::from_secs(0),
            })),
            health_check_interval: Duration::from_secs(30),
            max_connections: 1000,
            connection_timeout: Duration::from_secs(30),
            state_change_tx: Arc::new(state_tx),
            state_change_rx: state_rx,
            connected_clients: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(ServerConfig::default())),
        }
    }

    pub async fn get_config(&self) -> Result<ServerConfig, NexaError> {
        let config = self.config.read().await;
        Ok(config.clone())
    }

    pub async fn get_state(&self) -> ServerState {
        self.state.read().await.clone()
    }

    pub async fn get_bound_addr(&self) -> Option<std::net::SocketAddr> {
        *self.bound_addr.read().await
    }

    pub async fn get_active_connections(&self) -> u32 {
        *self.active_connections.read().await
    }

    pub async fn start(&self) -> Result<(), NexaError> {
        let mut state = self.state.write().await;
        if *state != ServerState::Stopped {
            return Err(NexaError::Server("Server is not in stopped state".to_string()));
        }
        *state = ServerState::Starting;
        drop(state);

        // Create runtime directory if it doesn't exist
        let config = self.config.read().await;
        tokio::fs::create_dir_all(&config.runtime_dir).await?;

        // Write PID file
        let pid = std::process::id().to_string();
        tokio::fs::write(&self.pid_file, pid).await?;

        // Create and bind TCP listener
        let listener = TcpListener::bind(&config.bind_addr).await?;
        let local_addr = listener.local_addr()?;
        *self.bound_addr.write().await = Some(local_addr);

        // Setup shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        // Start server loop
        let server = Arc::new(self.clone());
        let handle = tokio::spawn(async move {
            info!("Server starting on {}", local_addr);
            
            let mut interval = tokio::time::interval(server.health_check_interval);
            
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Shutdown signal received");
                        break;
                    }
                    Ok((socket, addr)) = listener.accept() => {
                        if let Err(e) = server.handle_connection(socket, addr).await {
                            error!("Failed to handle connection from {}: {}", addr, e);
                            let mut metrics = server.metrics.write().await;
                            metrics.failed_connections += 1;
                            metrics.last_error = Some(e.to_string());
                        }
                    }
                    _ = interval.tick() => {
                        server.check_health().await;
                    }
                }
            }
            
            Ok(())
        });

        *self.server_handle.write().await = Some(handle);
        *self.state.write().await = ServerState::Running;
        self.ready_notify.notify_waiters();

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), NexaError> {
        let mut state = self.state.write().await;
        if *state != ServerState::Running {
            return Err(NexaError::Server("Server is not running".to_string()));
        }
        *state = ServerState::Stopping;
        drop(state);

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }

        // Wait for server to stop with timeout
        let config = self.config.read().await;
        if let Some(handle) = self.server_handle.write().await.take() {
            match tokio::time::timeout(config.shutdown_timeout, handle).await {
                Ok(result) => {
                    if let Err(e) = result {
                        error!("Server task failed during shutdown: {}", e);
                    }
                }
                Err(_) => {
                    error!("Server shutdown timed out");
                }
            }
        }

        // Cleanup
        if self.pid_file.exists() {
            let _ = tokio::fs::remove_file(&self.pid_file).await;
        }

        *self.state.write().await = ServerState::Stopped;
        Ok(())
    }

    async fn handle_connection(&self, socket: TcpStream, addr: SocketAddr) -> Result<(), NexaError> {
        let active_conns = self.get_active_connections().await;
        
        if active_conns >= self.max_connections {
            return Err(NexaError::Server("Maximum connections reached".to_string()));
        }

        // Configure socket
        socket.set_nodelay(true)?;
        
        // Upgrade to WebSocket
        let ws_stream = tokio_tungstenite::accept_async(socket).await?;
        let (write, read) = ws_stream.split();
        
        // Update metrics and state
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_connections += 1;
            metrics.active_connections += 1;
        }
        *self.active_connections.write().await += 1;
        self.connected_clients.write().await.insert(addr, SystemTime::now());

        // Spawn connection handler
        let server = self.clone();
        tokio::spawn(async move {
            if let Err(e) = server.process_connection(read, write, addr).await {
                error!("Connection error for {}: {}", addr, e);
            }
            
            // Cleanup on disconnect
            server.connected_clients.write().await.remove(&addr);
            *server.active_connections.write().await -= 1;
            let mut metrics = server.metrics.write().await;
            metrics.active_connections -= 1;
        });

        Ok(())
    }

    async fn process_connection(
        &self,
        mut read: SplitStream<WebSocketStream<TcpStream>>,
        mut write: SplitSink<WebSocketStream<TcpStream>, Message>,
        addr: SocketAddr,
    ) -> Result<(), NexaError> {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(msg) => {
                    match msg {
                        Message::Text(text) => {
                            match serde_json::from_str(&text) {
                                Ok(message) => {
                                    if let Err(e) = Server::handle_client_message(&message, &mut write).await {
                                        error!("Failed to handle message from {}: {}", addr, e);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse message from {}: {}", addr, e);
                                }
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                Err(e) => {
                    error!("WebSocket error from {}: {}", addr, e);
                    break;
                }
            }
        }
        Ok(())
    }

    async fn handle_client_message(
        _message: &serde_json::Value,
        _write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    ) -> Result<(), NexaError> {
        // TODO: Implement message handling
        Ok(())
    }

    async fn check_health(&self) {
        let now = SystemTime::now();
        let mut clients = self.connected_clients.write().await;
        
        // Remove stale connections
        clients.retain(|_, last_seen| {
            now.duration_since(*last_seen)
                .map(|duration| duration < self.connection_timeout)
                .unwrap_or(false)
        });
        
        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.active_connections = clients.len() as u32;
        if let Ok(duration) = now.duration_since(metrics.start_time) {
            metrics.uptime = duration;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_lifecycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let pid_file = temp_dir.path().join("server.pid");
        let socket_path = temp_dir.path().join("server.sock");
        
        let server = Server::new(pid_file, socket_path);
        
        // Configure server to use a random available port
        {
            let mut config = server.config.write().await;
            config.bind_addr = "127.0.0.1:0".to_string();
        }
        
        // Test start
        assert!(server.start().await.is_ok());
        assert_eq!(server.get_state().await, ServerState::Running);
        
        // Test stop
        assert!(server.stop().await.is_ok());
        assert_eq!(server.get_state().await, ServerState::Stopped);
    }
} 