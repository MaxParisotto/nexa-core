mod config;

pub use config::ServerConfig;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{RwLock, Notify};
use tokio::net::{TcpListener, TcpStream};
use std::net::SocketAddr;
use tracing::{error, info, debug};
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
            _ => Err(NexaError::server(format!("Invalid server state: {}", s))),
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

#[derive(Debug)]
struct ServerInternalState {
    state: ServerState,
    shutdown_requested: bool,
}

#[derive(Clone, Debug)]
pub struct Server {
    pid_file: PathBuf,
    socket_path: PathBuf,
    bound_addr: Arc<RwLock<Option<std::net::SocketAddr>>>,
    state: Arc<RwLock<ServerInternalState>>,
    shutdown_tx: Arc<tokio::sync::broadcast::Sender<()>>,
    active_connections: Arc<RwLock<u32>>,
    server_handle: Arc<RwLock<Option<tokio::task::JoinHandle<Result<(), NexaError>>>>>,
    ready_notify: Arc<Notify>,
    metrics: Arc<RwLock<ServerMetrics>>,
    health_check_interval: Duration,
    max_connections: u32,
    connection_timeout: Duration,
    connected_clients: Arc<RwLock<HashMap<SocketAddr, SystemTime>>>,
    config: Arc<RwLock<ServerConfig>>,
}

impl Server {
    pub fn new(pid_file: PathBuf, socket_path: PathBuf) -> Self {
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);
        
        Self {
            pid_file,
            socket_path,
            bound_addr: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(ServerInternalState {
                state: ServerState::Stopped,
                shutdown_requested: false,
            })),
            shutdown_tx: Arc::new(shutdown_tx),
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
            connected_clients: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(ServerConfig::default())),
        }
    }

    pub async fn get_config(&self) -> Result<ServerConfig, NexaError> {
        let config = self.config.read().await;
        Ok(config.clone())
    }

    pub async fn set_config(&self, config: ServerConfig) -> Result<(), NexaError> {
        *self.config.write().await = config;
        Ok(())
    }

    pub async fn get_state(&self) -> ServerState {
        self.state.read().await.state.clone()
    }

    pub async fn get_bound_addr(&self) -> Option<std::net::SocketAddr> {
        *self.bound_addr.read().await
    }

    pub async fn get_active_connections(&self) -> u32 {
        *self.active_connections.read().await
    }

    pub async fn start(&self) -> Result<(), NexaError> {
        debug!("Starting server initialization");
        let mut state = self.state.write().await;
        if state.state != ServerState::Stopped {
            return Err(NexaError::server("Server is not in stopped state"));
        }
        state.state = ServerState::Starting;
        drop(state);

        // Create TCP listener
        debug!("Creating TCP listener");
        let config = self.config.read().await;
        let bind_addr = &config.bind_addr;
        debug!("Attempting to bind to {}", bind_addr);
        let listener = TcpListener::bind(bind_addr).await
            .map_err(|e| NexaError::server(format!("Failed to bind to {}: {}", bind_addr, e)))?;
        
        let local_addr = listener.local_addr()?;
        *self.bound_addr.write().await = Some(local_addr);
        debug!("Server bound to {}", local_addr);

        // Start server loop
        let server = Arc::new(self.clone());
        let handle = tokio::spawn(async move {
            info!("Server starting on {}", local_addr);
            
            let mut interval = tokio::time::interval(server.health_check_interval);
            let mut shutdown_rx = server.shutdown_tx.subscribe();
            debug!("Server loop initialized");
            
            // Set server state to running before starting the loop
            {
                let mut state = server.state.write().await;
                state.state = ServerState::Running;
                state.shutdown_requested = false;
                debug!("Server state set to running");
            }
            
            // Create a channel for signaling the accept loop to stop
            let (accept_stop_tx, accept_stop_rx) = tokio::sync::oneshot::channel();
            
            // Spawn the accept loop in a separate task
            let server_clone = server.clone();
            let accept_handle = tokio::spawn(async move {
                let mut accept_stop_rx = accept_stop_rx;
                loop {
                    tokio::select! {
                        accept_result = listener.accept() => {
                            match accept_result {
                                Ok((socket, addr)) => {
                                    let state = server_clone.state.read().await;
                                    if state.shutdown_requested {
                                        debug!("Rejecting connection during shutdown");
                                        continue;
                                    }
                                    drop(state);
                                    
                                    // Handle connection in a separate task
                                    let server = server_clone.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) = server.handle_connection(socket, addr).await {
                                            error!("Error handling connection: {}", e);
                                        }
                                    });
                                }
                                Err(e) => {
                                    error!("Error accepting connection: {}", e);
                                    // Break if listener is closed
                                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                                        break;
                                    }
                                }
                            }
                        }
                        _ = &mut accept_stop_rx => {
                            debug!("Accept loop received stop signal");
                            // Drop the listener to force any pending accepts to fail
                            drop(listener);
                            break;
                        }
                    }
                }
                debug!("Accept loop exited");
            });
            
            // Main server loop
            loop {
                tokio::select! {
                    Ok(()) = shutdown_rx.recv() => {
                        info!("Shutdown signal received in server loop");
                        // Set state to stopping
                        {
                            let mut state = server.state.write().await;
                            state.state = ServerState::Stopping;
                            state.shutdown_requested = true;
                            debug!("Server state set to stopping");
                        }
                        
                        // Signal the accept loop to stop
                        let _ = accept_stop_tx.send(());
                        
                        // Wait for accept loop to finish with timeout
                        match tokio::time::timeout(Duration::from_secs(5), accept_handle).await {
                            Ok(result) => {
                                if let Err(e) = result {
                                    error!("Accept loop failed during shutdown: {}", e);
                                } else {
                                    debug!("Accept loop completed successfully");
                                }
                            }
                            Err(_) => {
                                error!("Accept loop shutdown timed out");
                            }
                        }

                        // Set final state
                        {
                            let mut state = server.state.write().await;
                            state.state = ServerState::Stopped;
                            state.shutdown_requested = false;
                            debug!("Server state set to stopped");
                        }

                        // Clear bound address
                        *server.bound_addr.write().await = None;
                        debug!("Cleared bound address");

                        // Clear any remaining connections
                        {
                            let mut clients = server.connected_clients.write().await;
                            clients.clear();
                            *server.active_connections.write().await = 0;
                            debug!("Cleared all connections");
                        }

                        debug!("Server loop exited");
                        break;
                    }
                    _ = interval.tick() => {
                        let state = server.state.read().await;
                        if state.shutdown_requested {
                            debug!("Skipping health check during shutdown");
                            continue;
                        }
                        drop(state);
                        
                        // Perform health check
                        let now = SystemTime::now();
                        {
                            let mut clients = server.connected_clients.write().await;
                            clients.retain(|_, last_seen| {
                                now.duration_since(*last_seen)
                                    .map(|duration| duration < server.connection_timeout)
                                    .unwrap_or(false)
                            });
                            
                            // Update metrics
                            let mut metrics = server.metrics.write().await;
                            metrics.active_connections = clients.len() as u32;
                            if let Ok(duration) = now.duration_since(metrics.start_time) {
                                metrics.uptime = duration;
                            }
                        }
                    }
                }
            }
            
            Ok(())
        });

        // Store server handle
        *self.server_handle.write().await = Some(handle);
        debug!("Server handle stored");

        // Notify waiters
        self.ready_notify.notify_waiters();
        debug!("Server initialization completed");

        // Return success
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), NexaError> {
        debug!("Starting server shutdown sequence");
        // First check if we're already in a non-running state
        {
            let mut state = self.state.write().await;
            match state.state {
                ServerState::Stopped => {
                    debug!("Server is already stopped");
                    return Ok(());
                }
                ServerState::Stopping => {
                    debug!("Server is already in the process of stopping");
                    return Ok(());
                }
                ServerState::Running => {
                    debug!("Server is running, proceeding with shutdown");
                    state.state = ServerState::Stopping;
                    state.shutdown_requested = true;
                }
                _ => {
                    return Err(NexaError::server("Server is not in a state that can be stopped"));
                }
            }
        }

        // Send shutdown signal
        debug!("Broadcasting shutdown signal");
        let _ = self.shutdown_tx.send(());

        // Wait for server task to complete with timeout
        if let Some(handle) = self.server_handle.write().await.take() {
            debug!("Waiting for server task to complete");
            match tokio::time::timeout(Duration::from_secs(5), handle).await {
                Ok(result) => {
                    if let Err(e) = result {
                        error!("Server task failed during shutdown: {}", e);
                    } else {
                        debug!("Server task completed successfully");
                    }
                }
                Err(_) => {
                    error!("Server task shutdown timed out");
                }
            }
        }

        // Wait for server to stop with timeout
        let mut retries = 10;
        while retries > 0 {
            {
                let state = self.state.read().await;
                if state.state == ServerState::Stopped {
                    debug!("Server stopped successfully");
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
            retries -= 1;
        }

        if retries == 0 {
            error!("Server failed to stop within timeout");
            return Err(NexaError::system("Server failed to stop within timeout"));
        }

        // Clear bound address
        *self.bound_addr.write().await = None;
        debug!("Cleared bound address");

        // Clear any remaining connections
        {
            let mut clients = self.connected_clients.write().await;
            clients.clear();
            *self.active_connections.write().await = 0;
            debug!("Cleared all connections");
        }

        // Cleanup
        debug!("Cleaning up server resources");
        if self.pid_file.exists() {
            if let Err(e) = tokio::fs::remove_file(&self.pid_file).await {
                error!("Failed to remove PID file: {}", e);
            } else {
                debug!("PID file removed");
            }
        } else {
            debug!("No PID file to remove");
        }

        if self.socket_path.exists() {
            if let Err(e) = tokio::fs::remove_file(&self.socket_path).await {
                error!("Failed to remove socket file: {}", e);
            } else {
                debug!("Socket file removed");
            }
        } else {
            debug!("No socket file to remove");
        }

        Ok(())
    }

    pub async fn handle_connection(&self, socket: TcpStream, addr: SocketAddr) -> Result<(), NexaError> {
        let active_conns = *self.active_connections.read().await;
        
        if active_conns >= self.max_connections {
            return Err(NexaError::server("Maximum connections reached"));
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

    pub async fn check_health(&self) {
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
        
        // Test start
        assert!(server.start().await.is_ok());
        assert_eq!(server.get_state().await, ServerState::Running);
        
        // Test stop
        assert!(server.stop().await.is_ok());
        assert_eq!(server.get_state().await, ServerState::Stopped);
    }
} 