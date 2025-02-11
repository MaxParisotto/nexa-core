use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
    net::SocketAddr,
    str::FromStr,
    fs,
};
use tokio::{
    sync::RwLock,
    net::{TcpListener, TcpStream},
};
use log::{error, info, debug};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::protocol::Message,
    accept_async,
};
use futures::{
    stream::{SplitStream, SplitSink},
    StreamExt,
    SinkExt,
};
use uuid::Uuid;
use crate::error::NexaError;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerState {
    Starting,
    Running,
    Stopping,
    Stopped,
}

impl FromStr for ServerState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Starting" => Ok(ServerState::Starting),
            "Running" => Ok(ServerState::Running),
            "Stopping" => Ok(ServerState::Stopping),
            "Stopped" => Ok(ServerState::Stopped),
            _ => Err(format!("Invalid server state: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerMetrics {
    pub total_connections: u64,
    pub active_connections: u32,
    pub failed_connections: u64,
    pub last_error: Option<String>,
    pub uptime: Duration,
    pub start_time: Option<SystemTime>,
}

impl ServerMetrics {
    pub fn new() -> Self {
        Self {
            total_connections: 0,
            active_connections: 0,
            failed_connections: 0,
            last_error: None,
            uptime: Duration::from_secs(0),
            start_time: Some(SystemTime::now()),
        }
    }

    pub fn reset(&mut self) {
        self.total_connections = 0;
        self.active_connections = 0;
        self.failed_connections = 0;
        self.last_error = None;
        self.uptime = Duration::from_secs(0);
        self.start_time = None;
    }
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self {
            total_connections: 0,
            active_connections: 0,
            failed_connections: 0,
            last_error: None,
            uptime: Duration::from_secs(0),
            start_time: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Server {
    clients: Arc<RwLock<HashMap<Uuid, SystemTime>>>,
    port: u16,
    state: Arc<RwLock<ServerState>>,
    bound_addr: Arc<RwLock<Option<SocketAddr>>>,
    max_connections: usize,
    active_connections: Arc<RwLock<usize>>,
    connected_clients: Arc<RwLock<HashMap<SocketAddr, SystemTime>>>,
    metrics: Arc<RwLock<ServerMetrics>>,
    listener: Arc<RwLock<Option<TcpListener>>>,
    runtime_dir: PathBuf,
}

impl Server {
    pub fn new(pid_file: PathBuf, socket_path: PathBuf) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let runtime_dir = PathBuf::from(&home).join(".nexa");
        
        // Ensure runtime directory exists
        fs::create_dir_all(&runtime_dir).expect("Failed to create runtime directory");
        
        // Write PID file
        let pid = std::process::id();
        let _ = fs::write(&pid_file, pid.to_string());
        
        // Create Unix domain socket if on Unix
        #[cfg(unix)]
        if let Some(parent) = socket_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        
        let mut initial_metrics = ServerMetrics::new();
        initial_metrics.start_time = Some(SystemTime::now());
        
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            port: 8085,
            state: Arc::new(RwLock::new(ServerState::Stopped)),
            bound_addr: Arc::new(RwLock::new(None)),
            max_connections: 100,
            active_connections: Arc::new(RwLock::new(0)),
            connected_clients: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(initial_metrics)),
            listener: Arc::new(RwLock::new(None)),
            runtime_dir,
        }
    }

    async fn save_state(&self, state: ServerState) -> Result<(), Box<dyn std::error::Error>> {
        let state_file = self.runtime_dir.join("nexa.state");
        let state_str = match state {
            ServerState::Starting => "Starting",
            ServerState::Running => "Running",
            ServerState::Stopping => "Stopping",
            ServerState::Stopped => "Stopped",
        };
        fs::write(state_file, state_str)?;
        Ok(())
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.state.write().await;
        *state = ServerState::Starting;
        self.save_state(ServerState::Starting).await?;
        
        // Reset all state in a synchronized way
        {
            let mut metrics = self.metrics.write().await;
            *metrics = ServerMetrics::new();
            metrics.start_time = Some(SystemTime::now());
        }
        {
            let mut clients = self.clients.write().await;
            clients.clear();
        }
        {
            let mut connected = self.connected_clients.write().await;
            connected.clear();
        }
        *self.active_connections.write().await = 0;
        
        // Start TCP listener
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                *state = ServerState::Stopped;
                self.save_state(ServerState::Stopped).await?;
                return Err(NexaError::Server(format!("Failed to bind to address: {}", e)).into());
            }
        };
        
        let bound_addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(e) => {
                *state = ServerState::Stopped;
                self.save_state(ServerState::Stopped).await?;
                return Err(NexaError::Server(format!("Failed to get local address: {}", e)).into());
            }
        };
        
        *self.bound_addr.write().await = Some(bound_addr);
        *self.listener.write().await = Some(listener);
        
        info!("WebSocket server listening on: {}", bound_addr);
        
        *state = ServerState::Running;
        self.save_state(ServerState::Running).await?;
        drop(state);
        
        // Start server loop in a separate task
        let server = self.clone();
        tokio::spawn(async move {
            if let Some(listener) = &*server.listener.read().await {
                loop {
                    let state = server.state.read().await;
                    if *state != ServerState::Running {
                        break;
                    }
                    drop(state);
                    
                    match listener.accept().await {
                        Ok((stream, addr)) => {
                            info!("New connection from: {}", addr);
                            let server = server.clone();
                            tokio::spawn(async move {
                                if let Err(e) = server.handle_connection(stream, addr).await {
                                    error!("Error handling connection: {}", e);
                                    let mut metrics = server.metrics.write().await;
                                    metrics.failed_connections += 1;
                                    metrics.last_error = Some(e.to_string());
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                            let mut metrics = server.metrics.write().await;
                            metrics.failed_connections += 1;
                            metrics.last_error = Some(e.to_string());
                        }
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.state.write().await;
        *state = ServerState::Stopping;
        self.save_state(ServerState::Stopping).await?;
        
        // Clear listener to stop accepting new connections
        if let Some(listener) = self.listener.write().await.take() {
            drop(listener);
        }
        
        // Reset metrics
        let mut metrics = self.metrics.write().await;
        metrics.reset();
        drop(metrics);
        
        *state = ServerState::Stopped;
        self.save_state(ServerState::Stopped).await?;
        
        // Clean up files
        let _ = fs::remove_file(self.runtime_dir.join("nexa.state"));
        let _ = fs::remove_file(self.runtime_dir.join("nexa.pid"));
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_state(&self) -> ServerState {
        self.state.read().await.clone()
    }

    pub async fn get_active_connections(&self) -> usize {
        let clients = self.connected_clients.read().await;
        clients.len()
    }

    #[allow(dead_code)]
    pub async fn get_metrics(&self) -> ServerMetrics {
        self.metrics.write().await.clone()
    }

    async fn handle_connection(&self, socket: TcpStream, addr: SocketAddr) -> Result<(), NexaError> {
        let active_conns = self.get_active_connections().await;
        
        if active_conns >= self.max_connections {
            let mut metrics = self.metrics.write().await;
            metrics.failed_connections += 1;
            metrics.last_error = Some("Maximum connections reached".to_string());
            error!("Connection rejected: maximum connections ({}) reached", self.max_connections);
            return Err(NexaError::Server("Maximum connections reached".to_string()));
        }

        if let Err(e) = socket.set_nodelay(true) {
            let mut metrics = self.metrics.write().await;
            metrics.failed_connections += 1;
            metrics.last_error = Some(e.to_string());
            error!("Failed to set TCP_NODELAY: {}", e);
            return Err(NexaError::from(e));
        }
        
        let ws_stream = match accept_async(socket).await {
            Ok(stream) => stream,
            Err(e) => {
                let mut metrics = self.metrics.write().await;
                metrics.failed_connections += 1;
                metrics.last_error = Some(e.to_string());
                error!("WebSocket handshake failed: {}", e);
                return Err(NexaError::from(e));
            }
        };
        
        let (write_half, read_half) = ws_stream.split();
        
        // Update metrics and connection tracking
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_connections += 1;
            metrics.active_connections += 1;
            info!("New connection established - Total: {}, Active: {}, Failed: {}", 
                metrics.total_connections, 
                metrics.active_connections, 
                metrics.failed_connections
            );
        }
        {
            let client_id = Uuid::new_v4();
            self.clients.write().await.insert(client_id, SystemTime::now());
            self.connected_clients.write().await.insert(addr, SystemTime::now());
            *self.active_connections.write().await += 1;
        }

        debug!("Processing WebSocket connection from {}", addr);
        self.process_ws_connection(read_half, write_half, addr).await?;

        // Cleanup connection
        {
            self.connected_clients.write().await.remove(&addr);
            *self.active_connections.write().await = self.active_connections.write().await.saturating_sub(1);
            let mut metrics = self.metrics.write().await;
            metrics.active_connections = metrics.active_connections.saturating_sub(1);
            debug!("Connection closed - Active: {}", metrics.active_connections);
        }

        Ok(())
    }

    async fn process_ws_connection(
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
                            if let Err(e) = write.send(Message::Text(text)).await {
                                error!("Failed to send message: {}", e);
                                break;
                            }
                        }
                        Message::Close(_) => break,
                        Message::Ping(data) => {
                            if let Err(e) = write.send(Message::Pong(data)).await {
                                error!("Failed to send pong: {}", e);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    error!("Error receiving message from {}: {}", addr, e);
                    break;
                }
            }
        }
        Ok(())
    }
} 