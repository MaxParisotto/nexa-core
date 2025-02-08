use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, atomic::{AtomicBool, Ordering}},
    time::{Duration, SystemTime},
    net::SocketAddr,
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

static SERVER_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, PartialEq)]
pub enum ServerState {
    Starting,
    Running,
    Stopping,
    Stopped,
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
            start_time: None,
        }
    }

    pub fn update_uptime(&mut self) {
        if let Some(start) = self.start_time {
            if let Ok(duration) = SystemTime::now().duration_since(start) {
                self.uptime = duration;
            }
        }
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
}

impl Server {
    pub fn new(_pid_file: PathBuf, _socket_path: PathBuf) -> Self {
        let mut initial_metrics = ServerMetrics::new();
        initial_metrics.start_time = Some(SystemTime::now());
        initial_metrics.update_uptime();

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
        }
    }

    pub async fn start(&self) -> Result<(), NexaError> {
        let mut state = self.state.write().await;
        if *state == ServerState::Running {
            return Err(NexaError::Server("Server is already running".to_string()));
        }
        *state = ServerState::Starting;
        drop(state);

        // Reset metrics
        let mut metrics = self.metrics.write().await;
        *metrics = ServerMetrics::new();
        metrics.start_time = Some(SystemTime::now());
        metrics.update_uptime();
        drop(metrics);

        // Start TCP listener
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        let bound_addr = listener.local_addr()?;
        *self.bound_addr.write().await = Some(bound_addr);
        *self.listener.write().await = Some(listener);
        
        info!("WebSocket server listening on: {}", bound_addr);

        // Update state to running
        let mut state = self.state.write().await;
        *state = ServerState::Running;
        drop(state);

        // Start server loop in a separate task
        let server = self.clone();
        tokio::spawn(async move {
            if let Some(listener) = &*server.listener.read().await {
                while *server.state.read().await == ServerState::Running {
                    if let Ok((stream, addr)) = listener.accept().await {
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
                }
            }
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), NexaError> {
        let mut state = self.state.write().await;
        if *state == ServerState::Stopped {
            return Err(NexaError::Server("Server is not running".to_string()));
        }
        *state = ServerState::Stopping;
        drop(state);

        // Clear listener
        *self.listener.write().await = None;

        // Reset metrics
        let mut metrics = self.metrics.write().await;
        *metrics = ServerMetrics::default();
        drop(metrics);

        // Clear bound address
        *self.bound_addr.write().await = None;

        // Clear connected clients
        self.connected_clients.write().await.clear();
        *self.active_connections.write().await = 0;

        // Update state to stopped
        let mut state = self.state.write().await;
        *state = ServerState::Stopped;
        
        Ok(())
    }

    pub async fn get_state(&self) -> ServerState {
        self.state.read().await.clone()
    }

    pub async fn get_active_connections(&self) -> usize {
        self.clients.read().await.len()
    }

    pub async fn get_metrics(&self) -> ServerMetrics {
        let mut metrics = self.metrics.write().await;
        metrics.update_uptime();
        metrics.clone()
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
        *self.active_connections.write().await += 1;
        self.connected_clients.write().await.insert(addr, SystemTime::now());

        debug!("Processing WebSocket connection from {}", addr);
        self.process_ws_connection(read_half, write_half, addr).await?;

        self.connected_clients.write().await.remove(&addr);
        *self.active_connections.write().await -= 1;
        {
            let mut metrics = self.metrics.write().await;
            metrics.active_connections -= 1;
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