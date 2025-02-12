use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
    fs,
    str::FromStr,
    net::{SocketAddr, IpAddr},
};
use tokio::{
    sync::{RwLock, broadcast},
    task::JoinHandle,
    net::TcpListener,
};
use log::{error, info, warn};
use serde::{Serialize, Deserialize};
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};
use tracing::{info_span, Instrument};
use crate::{
    cli::{CliError, CliHandler},
    api::ApiServer,
    config::ServerConfig,
};
use chrono;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

impl std::fmt::Display for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerState::Starting => write!(f, "Starting"),
            ServerState::Running => write!(f, "Running"),
            ServerState::Stopping => write!(f, "Stopping"),
            ServerState::Stopped => write!(f, "Stopped"),
            ServerState::Error => write!(f, "Error"),
        }
    }
}

impl FromStr for ServerState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Starting" => Ok(ServerState::Starting),
            "Running" => Ok(ServerState::Running),
            "Stopping" => Ok(ServerState::Stopping),
            "Stopped" => Ok(ServerState::Stopped),
            "Error" => Ok(ServerState::Error),
            _ => Err(format!("Invalid server state: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMetrics {
    pub total_connections: u64,
    pub active_connections: u32,
    pub failed_connections: u64,
    pub last_error: Option<String>,
    pub uptime: Duration,
    pub start_time: Option<SystemTime>,
    pub http_port: u16,
    pub version: String,
    pub build_info: BuildInfo,
    pub endpoints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    pub version: String,
    pub git_hash: Option<String>,
    pub build_timestamp: String,
    pub rust_version: String,
    pub target_os: String,
    pub target_arch: String,
}

impl ServerMetrics {
    pub fn new() -> Self {
        // Get target info at runtime
        let target_os = std::env::consts::OS.to_string();
        let target_arch = std::env::consts::ARCH.to_string();

        Self {
            total_connections: 0,
            active_connections: 0,
            failed_connections: 0,
            last_error: None,
            uptime: Duration::from_secs(0),
            start_time: Some(SystemTime::now()),
            http_port: 3000,
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_info: BuildInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                git_hash: option_env!("GIT_HASH").map(String::from),
                build_timestamp: chrono::Utc::now().to_rfc3339(),
                rust_version: rustc_version_runtime::version().to_string(),
                target_os,
                target_arch,
            },
            endpoints: vec![
                "/".to_string(),
                "/api/server/status".to_string(),
                "/swagger-ui/".to_string(),
                "/api-docs/openapi.json".to_string(),
            ],
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
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Server {
    state: Arc<RwLock<ServerState>>,
    metrics: Arc<RwLock<ServerMetrics>>,
    runtime_dir: PathBuf,
    server_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    shutdown_tx: broadcast::Sender<()>,
    config: Arc<RwLock<ServerConfig>>,
}

impl Server {
    pub fn new(pid_file: PathBuf, _socket_path: PathBuf) -> Self {
        let runtime_dir = pid_file.parent().unwrap_or(&pid_file).to_path_buf();
        let (shutdown_tx, _) = broadcast::channel(1);
        
        // Load configuration with proper error handling
        let config = match ServerConfig::load() {
            Ok(config) => config,
            Err(e) => {
                warn!("Failed to load config: {}, using defaults", e);
                ServerConfig::default()
            }
        };
        
        Self {
            state: Arc::new(RwLock::new(ServerState::Stopped)),
            metrics: Arc::new(RwLock::new(ServerMetrics::new())),
            runtime_dir,
            server_handle: Arc::new(RwLock::new(None)),
            shutdown_tx,
            config: Arc::new(RwLock::new(config)),
        }
    }

    pub async fn start(&self) -> Result<(), CliError> {
        let span = info_span!("server_start");
        async {
            let mut state = self.state.write().await;
            if *state == ServerState::Running {
                return Err(CliError::new("Server is already running".to_string()));
            }
            *state = ServerState::Starting;
            self.save_state(ServerState::Starting).await?;
            drop(state);

            // Initialize metrics
            {
                let mut metrics = self.metrics.write().await;
                metrics.start_time = Some(SystemTime::now());
            }

            // Get configuration
            let config = self.config.read().await;
            
            // Parse host address
            let addr = match config.host.parse::<IpAddr>() {
                Ok(ip) => SocketAddr::new(ip, config.port),
                Err(e) => {
                    error!("Failed to parse host address: {}", e);
                    let mut state = self.state.write().await;
                    *state = ServerState::Error;
                    self.save_state(ServerState::Error).await?;
                    return Err(CliError::new(format!("Invalid host address: {}", e)));
                }
            };

            // Create API server with Swagger UI
            let api = ApiServer::new(CliHandler::new());
            let router = api.router()
                .route("/", axum::routing::get(|| async {
                    axum::response::Redirect::permanent("/swagger-ui/")
                }))
                .layer(
                    CorsLayer::new()
                        .allow_origin(tower_http::cors::Any)
                        .allow_methods(tower_http::cors::Any)
                        .allow_headers(tower_http::cors::Any)
                        .max_age(Duration::from_secs(3600))
                )
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|request: &axum::http::Request<_>| {
                            tracing::info_span!(
                                "http_request",
                                method = %request.method(),
                                uri = %request.uri(),
                                version = ?request.version(),
                            )
                        })
                );

            // Bind server with proper error handling
            let listener = match TcpListener::bind(&addr).await {
                Ok(listener) => {
                    info!("Server listening on {}", addr);
                    info!("API Documentation: http://{}/swagger-ui/", addr);
                    info!("OpenAPI Spec: http://{}/api-docs/openapi.json", addr);
                    listener
                }
                Err(e) => {
                    error!("Failed to bind server: {}", e);
                    let mut state = self.state.write().await;
                    *state = ServerState::Error;
                    self.save_state(ServerState::Error).await?;
                    return Err(CliError::new(format!("Failed to start server: {}", e)));
                }
            };

            // Create shutdown channel
            let mut notify_shutdown_rx = self.shutdown_tx.subscribe();

            // Start metrics update task
            let metrics = self.metrics.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    let mut metrics = metrics.write().await;
                    metrics.update_uptime();
                }
            });

            // Create server task
            let server_task = axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = notify_shutdown_rx.recv().await;
                    info!("Received shutdown signal, stopping server");
                });

            // Start server in background task
            let handle = tokio::spawn(async move {
                info!("Starting HTTP server");
                if let Err(e) = server_task.await {
                    error!("Server error: {}", e);
                }
                info!("HTTP server stopped");
            });

            // Store server handle
            {
                let mut server_handle = self.server_handle.write().await;
                *server_handle = Some(handle);
            }

            // Wait a bit to ensure server is actually running
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Update state to running
            let mut state = self.state.write().await;
            *state = ServerState::Running;
            self.save_state(ServerState::Running).await?;

            // Handle Ctrl+C
            let shutdown_tx = self.shutdown_tx.clone();
            tokio::spawn(async move {
                tokio::signal::ctrl_c().await.ok();
                info!("Received Ctrl+C signal");
                let _ = shutdown_tx.send(());
            });

            // Keep the server running until shutdown is requested
            let mut shutdown_rx = self.shutdown_tx.subscribe();
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Main process received shutdown signal");
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Main process received Ctrl+C");
                    let _ = self.shutdown_tx.send(());
                }
            }

            Ok(())
        }.instrument(span).await
    }

    pub async fn stop(&self) -> Result<(), CliError> {
        let span = info_span!("server_stop");
        async {
            let mut state = self.state.write().await;
            *state = ServerState::Stopping;
            self.save_state(ServerState::Stopping).await?;
            
            // Signal server shutdown
            if let Err(e) = self.shutdown_tx.send(()) {
                error!("Failed to send shutdown signal: {}", e);
            }
            info!("Sent shutdown signal to server");
            
            // Wait for server to stop
            {
                let mut server_handle = self.server_handle.write().await;
                if let Some(handle) = server_handle.take() {
                    match tokio::time::timeout(Duration::from_secs(5), handle).await {
                        Ok(_) => info!("Server stopped gracefully"),
                        Err(_) => {
                            warn!("Server shutdown timed out");
                        }
                    }
                }
            }
            
            // Reset metrics
            let mut metrics = self.metrics.write().await;
            metrics.reset();
            drop(metrics);
            
            *state = ServerState::Stopped;
            self.save_state(ServerState::Stopped).await?;
            
            Ok(())
        }.instrument(span).await
    }

    async fn save_state(&self, state: ServerState) -> Result<(), CliError> {
        let span = info_span!("save_state", state = ?state);
        async {
            let state_file = self.runtime_dir.join("nexa.state");
            let pid_file = self.runtime_dir.join("nexa.pid");
            
            // Create runtime directory if it doesn't exist
            if !self.runtime_dir.exists() {
                fs::create_dir_all(&self.runtime_dir).map_err(|e| CliError::new(e.to_string()))?;
            }
            
            // Write state file atomically using a temporary file
            let temp_state = state_file.with_extension("tmp");
            fs::write(&temp_state, state.to_string()).map_err(|e| CliError::new(e.to_string()))?;
            fs::rename(&temp_state, &state_file).map_err(|e| CliError::new(e.to_string()))?;
            
            // Handle PID file based on state
            match state {
                ServerState::Running => {
                    let temp_pid = pid_file.with_extension("tmp");
                    fs::write(&temp_pid, std::process::id().to_string()).map_err(|e| CliError::new(e.to_string()))?;
                    fs::rename(&temp_pid, &pid_file).map_err(|e| CliError::new(e.to_string()))?;
                }
                ServerState::Stopped | ServerState::Error => {
                    let _ = fs::remove_file(&state_file);
                    let _ = fs::remove_file(&pid_file);
                }
                _ => {}
            }
            
            Ok(())
        }.instrument(span).await
    }
} 