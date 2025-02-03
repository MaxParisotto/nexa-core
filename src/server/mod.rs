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
use log::error;
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
}

impl ServerMetrics {
    pub fn new() -> Self {
        Self {
            total_connections: 0,
            active_connections: 0,
            failed_connections: 0,
            last_error: None,
            uptime: Duration::from_secs(0),
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
}

impl Server {
    pub fn new(_pid_file: PathBuf, _socket_path: PathBuf) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            port: 8080,
            state: Arc::new(RwLock::new(ServerState::Stopped)),
            bound_addr: Arc::new(RwLock::new(None)),
            max_connections: 100,
            active_connections: Arc::new(RwLock::new(0)),
            connected_clients: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(ServerMetrics::new())),
        }
    }

    pub async fn start(&self) -> Result<(), NexaError> {
        if SERVER_RUNNING.load(Ordering::SeqCst) {
            return Err(NexaError::System("Server is already running".to_string()));
        }

        *self.state.write().await = ServerState::Starting;

        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        let bound_addr = listener.local_addr()?;
        *self.bound_addr.write().await = Some(bound_addr);
        
        error!("WebSocket server listening on: {}", bound_addr);
        
        SERVER_RUNNING.store(true, Ordering::SeqCst);
        *self.state.write().await = ServerState::Running;
        
        while SERVER_RUNNING.load(Ordering::SeqCst) {
            if let Ok((stream, addr)) = listener.accept().await {
                error!("New connection from: {}", addr);
                let server = self.clone();
                tokio::spawn(async move {
                    if let Err(e) = server.handle_connection(stream, addr).await {
                        error!("Error handling connection: {}", e);
                    }
                });
            }
        }
        
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), NexaError> {
        if !SERVER_RUNNING.load(Ordering::SeqCst) {
            return Err(NexaError::System("Server is not running".to_string()));
        }
        
        *self.state.write().await = ServerState::Stopping;
        SERVER_RUNNING.store(false, Ordering::SeqCst);
        *self.state.write().await = ServerState::Stopped;
        *self.bound_addr.write().await = None;
        
        Ok(())
    }

    pub async fn get_state(&self) -> ServerState {
        self.state.read().await.clone()
    }

    pub async fn get_active_connections(&self) -> usize {
        self.clients.read().await.len()
    }

    pub async fn get_metrics(&self) -> ServerMetrics {
        self.metrics.read().await.clone()
    }

    async fn handle_connection(&self, socket: TcpStream, addr: SocketAddr) -> Result<(), NexaError> {
        let active_conns = self.get_active_connections().await;
        
        if active_conns >= self.max_connections {
            return Err(NexaError::Server("Maximum connections reached".to_string()));
        }

        socket.set_nodelay(true)?;
        
        let ws_stream = accept_async(socket).await?;
        let (write_half, read_half) = ws_stream.split();
        
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_connections += 1;
            metrics.active_connections += 1;
        }
        *self.active_connections.write().await += 1;
        self.connected_clients.write().await.insert(addr, SystemTime::now());

        self.process_ws_connection(read_half, write_half, addr).await?;

        self.connected_clients.write().await.remove(&addr);
        *self.active_connections.write().await -= 1;
        let mut metrics = self.metrics.write().await;
        metrics.active_connections -= 1;

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

pub async fn get_status() -> Result<String, NexaError> {
    let mut status = String::new();
    
    // Get system info
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    
    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let memory_used = sys.used_memory();
    let memory_total = sys.total_memory();
    let memory_usage = (memory_used as f32 / memory_total as f32) * 100.0;

    status.push_str(&format!("System Status:\n"));
    status.push_str(&format!("CPU Usage: {:.1}%\n", cpu_usage));
    status.push_str(&format!("Memory Usage: {:.1}%\n", memory_usage));
    
    Ok(status)
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub health_check_interval: std::time::Duration,
    pub connection_timeout: std::time::Duration,
}

// Global server instance
pub static SERVER: once_cell::sync::Lazy<Arc<Server>> = once_cell::sync::Lazy::new(|| {
    Arc::new(Server::new(
        PathBuf::from("/tmp/nexa.pid"),
        PathBuf::from("/tmp/nexa.sock")
    ))
}); 