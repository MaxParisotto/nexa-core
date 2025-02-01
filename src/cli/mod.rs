//! CLI Handler for Nexa Utils
//! 
//! Provides command-line interface functionality for:
//! - Starting/stopping the MCP server
//! - Monitoring system status
//! - Managing agents

use clap::{Parser, Subcommand};
use tracing::{debug, error, info, warn};
use crate::mcp::ServerControl;
use crate::monitoring::SystemHealth;
use crate::mcp::server::ServerState;
use std::path::PathBuf;
use crate::error::NexaError;
use std::time::Duration;
use std::cmp::min;
use sysinfo;
use std::error::Error;
use crate::config::Config;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Nexa Core server
    Start {
        /// Port to listen on
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
        
        /// Host to bind to
        #[arg(short, long, default_value = "127.0.0.1")]
        host: String,
    },
    
    /// Stop the Nexa Core server
    Stop,
    
    /// Get the status of the Nexa Core server
    Status,

    /// Manage server configuration
    Config {
        #[command(subcommand)]
        cmd: ConfigCommands,
    },

    /// Manage cluster operations
    Cluster {
        #[command(subcommand)]
        cmd: ClusterCommands,
    },

    /// Monitor server metrics and health
    Monitor {
        #[command(subcommand)]
        cmd: MonitorCommands,
    },

    /// Manage active connections
    Connections {
        #[command(subcommand)]
        cmd: ConnectionCommands,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show,
    /// Set configuration value
    Set {
        key: String,
        value: String,
    },
    /// Reset configuration to defaults
    Reset,
}

#[derive(Subcommand)]
enum ClusterCommands {
    /// Show cluster status
    Status,
    /// Join an existing cluster
    Join {
        /// Address of any cluster node
        address: String,
    },
    /// Leave the cluster gracefully
    Leave,
    /// List all cluster nodes
    Nodes,
}

#[derive(Subcommand)]
enum MonitorCommands {
    /// Show detailed metrics
    Metrics,
    /// Show health status
    Health,
    /// View or follow logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },
}

#[derive(Subcommand)]
enum ConnectionCommands {
    /// List active connections
    List,
    /// Set connection limits
    Limit {
        /// Maximum number of connections
        max: u32,
    },
    /// Disconnect client(s)
    Disconnect {
        /// Client ID or 'all'
        target: String,
    },
}

pub struct CliController {
    server_control: ServerControl,
    pid_file: PathBuf,
    state_file: PathBuf,
    socket_path: PathBuf,
}

impl CliController {
    pub fn new() -> Self {
        let runtime_dir = std::env::var("TMPDIR")
            .map(|dir| dir.trim_end_matches('/').to_string())
            .unwrap_or_else(|_| "/tmp".to_string());
        let runtime_dir = PathBuf::from(runtime_dir);
        let pid_file = runtime_dir.join("nexa.pid");
        let socket_path = runtime_dir.join("nexa.sock");
        let state_file = pid_file.with_extension("state");
        debug!("Using runtime directory for PID file: {:?}", pid_file);
        
        // Create parent directories if they don't exist
        if let Some(parent) = pid_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Some(parent) = socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Some(parent) = state_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
            
        Self {
            server_control: ServerControl::new(pid_file.clone(), socket_path.clone()),
            pid_file,
            state_file,
            socket_path,
        }
    }

    pub fn new_with_paths(pid_file: PathBuf, socket_path: PathBuf, state_file: PathBuf) -> Self {
        debug!("Using custom paths - PID file: {:?}, Socket: {:?}, State: {:?}", pid_file, socket_path, state_file);
        Self {
            server_control: ServerControl::new(pid_file.clone(), socket_path.clone()),
            pid_file,
            state_file,
            socket_path,
        }
    }

    fn check_process_exists(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            use nix::sys::signal;
            use nix::unistd::Pid;

            // First try using kill(0) to check process existence
            if signal::kill(Pid::from_raw(pid as i32), None).is_ok() {
                debug!("Process {} exists (kill check)", pid);
                return true;
            }

            // If kill failed, try platform-specific checks
            #[cfg(target_os = "linux")]
            {
                if std::path::Path::new(&format!("/proc/{}/stat", pid)).exists() {
                    debug!("Process {} exists (proc check)", pid);
                    return true;
                }
            }

            #[cfg(target_os = "macos")]
            {
                use std::process::Command;
                if let Ok(output) = Command::new("ps")
                    .arg("-p")
                    .arg(pid.to_string())
                    .output()
                {
                    let exists = output.status.success() && 
                                String::from_utf8_lossy(&output.stdout).lines().count() > 1;
                    if exists {
                        debug!("Process {} exists (ps check)", pid);
                        return true;
                    }
                }
            }

            debug!("Process {} does not exist", pid);
            false
        }

        #[cfg(not(unix))]
        {
            false
        }
    }

    pub async fn is_server_running(&self) -> bool {
        // First check if PID file exists
        if let Err(_) = tokio::fs::metadata(&self.pid_file).await {
            debug!("PID file does not exist");
            return false;
        }

        // Read and validate PID
        let pid_str = match tokio::fs::read_to_string(&self.pid_file).await {
            Ok(content) => content.trim().to_string(),
            Err(e) => {
                debug!("Failed to read PID file: {}", e);
                return false;
            }
        };

        let pid = match pid_str.parse::<u32>() {
            Ok(p) => p,
            Err(e) => {
                debug!("Invalid PID in file: {}", e);
                return false;
            }
        };

        // Check if process exists
        if !self.check_process_exists(pid) {
            debug!("Server process {} does not exist", pid);
            // Clean up stale files since the process is gone
            let this = self.clone();
            tokio::spawn(async move {
                if let Err(e) = this.cleanup_files().await {
                    debug!("Failed to clean up stale files: {}", e);
                }
            });
            return false;
        }

        // Check state file
        let state_str = match tokio::fs::read_to_string(&self.state_file).await {
            Ok(content) => content.trim().to_string(),
            Err(e) => {
                debug!("Failed to read state file: {}", e);
                return false;
            }
        };

        match state_str.parse::<ServerState>() {
            Ok(state) => {
                debug!("Server state from file: {}", state);
                matches!(state, ServerState::Running | ServerState::Starting)
            }
            Err(e) => {
                debug!("Invalid state in file: {}", e);
                false
            }
        }
    }

    /// Get the current server state
    pub async fn get_server_state(&self) -> Result<ServerState, NexaError> {
        self.server_control.get_state().await
    }

    pub async fn cleanup_files(&self) -> Result<(), NexaError> {
        debug!("Starting file cleanup");
        let mut cleanup_needed = false;

        // Try to remove PID file
        if let Ok(_) = tokio::fs::metadata(&self.pid_file).await {
            cleanup_needed = true;
            if let Err(e) = tokio::fs::remove_file(&self.pid_file).await {
                debug!("Failed to remove PID file: {}", e);
            } else {
                debug!("Successfully removed PID file");
            }
        }

        // Try to remove state file
        if let Ok(_) = tokio::fs::metadata(&self.state_file).await {
            cleanup_needed = true;
            if let Err(e) = tokio::fs::remove_file(&self.state_file).await {
                debug!("Failed to remove state file: {}", e);
            } else {
                debug!("Successfully removed state file");
            }
        }

        // Try to remove socket file
        if let Ok(_) = tokio::fs::metadata(&self.socket_path).await {
            cleanup_needed = true;
            if let Err(e) = tokio::fs::remove_file(&self.socket_path).await {
                debug!("Failed to remove socket file: {}", e);
            } else {
                debug!("Successfully removed socket file");
            }
        }

        if cleanup_needed {
            debug!("File cleanup completed");
        } else {
            debug!("No files needed cleanup");
        }

        Ok(())
    }

    pub async fn handle_start(&self, config: &Option<String>) -> Result<(), NexaError> {
        info!("Starting MCP server");

        // Check if server is already running
        if self.is_server_running().await {
            error!("Server is already running");
            return Err(NexaError::system("Server is already running"));
        }

        // Clean up any stale files
        debug!("Starting file cleanup");
        if let Err(e) = self.cleanup_files().await {
            error!("Failed to clean up stale files: {}", e);
            return Err(e);
        }

        // Start the server
        self.server_control.start(config.as_deref()).await?;

        // Wait for server to be ready
        let mut retries = 10;
        let mut delay = Duration::from_millis(100);
        while retries > 0 {
            debug!("Checking server state (retries left: {}, delay: {:?})", retries, delay);
            let state = match tokio::fs::read_to_string(&self.state_file).await {
                Ok(content) => content.trim().parse::<ServerState>()?,
                Err(e) => {
                    debug!("Failed to read state file: {}", e);
                    ServerState::Stopped
                }
            };
            debug!("Server state: {}", state);

            if state == ServerState::Running {
                info!("Server started successfully");
                return Ok(());
            }

            tokio::time::sleep(delay).await;
            retries -= 1;
            delay = min(delay * 2, Duration::from_secs(1));
        }

        error!("Server failed to start within timeout");
        Err(NexaError::system("Server failed to start within timeout"))
    }

    pub async fn handle_stop(&self) -> Result<(), NexaError> {
        info!("Stopping MCP server");
        
        // Check if server is running first
        if !self.is_server_running().await {
            error!("Server is not running");
            return Err(NexaError::system("Server is not running"));
        }

        // Try to stop the server with exponential backoff
        let mut retries = 5;
        let mut retry_delay = Duration::from_millis(200);
        let max_delay = Duration::from_secs(2);

        while retries > 0 {
            debug!("Attempting to stop server (retries left: {}, delay: {}ms)", 
                  retries, retry_delay.as_millis());

            match self.server_control.stop().await {
                Ok(_) => {
                    // Wait for server to fully stop with timeout
                    let mut stop_retries = 10;
                    let mut stop_delay = Duration::from_millis(100);

                    while stop_retries > 0 {
                        debug!("Checking if server has stopped (retries left: {})", stop_retries);
                        if !self.is_server_running().await {
                            // Clean up any remaining files
                            if let Err(e) = self.cleanup_files().await {
                                debug!("Failed to clean up files after stop: {}", e);
                            }
                            info!("Server stopped successfully");
                            return Ok(());
                        }
                        tokio::time::sleep(stop_delay).await;
                        stop_delay = min(stop_delay * 2, Duration::from_secs(1));
                        stop_retries -= 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to stop server (attempt {}): {}", 6 - retries, e);
                }
            }

            tokio::time::sleep(retry_delay).await;
            retry_delay = min(retry_delay * 2, max_delay);
            retries -= 1;
        }

        // If we get here, all retries failed
        error!("Failed to stop server after retries");
        // Try to force cleanup as a last resort
        if let Err(e) = self.cleanup_files().await {
            debug!("Failed to clean up files after failed stop: {}", e);
        }
        Err(NexaError::system("Failed to stop server after retries"))
    }

    pub async fn handle_status(&self) -> Result<String, NexaError> {
        info!("Getting system status");
        
        let mut output = String::new();
        output.push_str("\nSystem Status:\n");
        
        // Always show resource usage
        let sys = sysinfo::System::new_all();
        output.push_str("\nResource Usage:\n");
        output.push_str(&format!("  CPU: {:.1}%\n", sys.global_cpu_info().cpu_usage()));
        output.push_str(&format!("  Memory: {:.1}%\n", 
            (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0));
        
        if self.is_server_running().await {
            output.push_str("\nServer Status: 🟢 Running\n");
            
            // Get metrics
            if let Ok(metrics) = self.server_control.get_metrics().await {
                output.push_str(&format!("\nActive Connections: {}\n", metrics.active_agents));
            }
            
            // Get additional status info
            if let Ok(health) = self.server_control.check_health().await {
                output.push_str(&format!("\nSystem Health: {}\n", health.message));
            }
            
            if let Ok(addr) = self.server_control.get_bound_addr().await {
                output.push_str(&format!("\nListening on: {}\n", addr));
            }
        } else {
            output.push_str("\nServer Status: 🔴 Stopped\n");
            output.push_str("\nServer is not running. Start it with 'nexa start'\n");
        }
        
        Ok(output)
    }

    pub async fn get_bound_addr(&self) -> Result<std::net::SocketAddr, NexaError> {
        self.server_control.get_bound_addr().await
    }

    /// Check system health
    pub async fn check_health(&self) -> Result<SystemHealth, NexaError> {
        self.server_control.check_health().await
    }

    pub fn get_pid_file_path(&self) -> PathBuf {
        self.pid_file.clone()
    }

    pub fn get_socket_path(&self) -> PathBuf {
        self.socket_path.clone()
    }

    pub fn get_state_file_path(&self) -> PathBuf {
        self.state_file.clone()
    }

    async fn handle_monitor_metrics(&self) -> Result<String, NexaError> {
        let mut output = String::new();
        output.push_str("\nDetailed System Metrics:\n");
        
        // System metrics
        let sys = sysinfo::System::new_all();
        output.push_str("\n🖥️  Hardware Metrics:\n");
        output.push_str(&format!("  CPU Usage: {:.1}%\n", sys.global_cpu_info().cpu_usage()));
        output.push_str(&format!("  Memory Used: {:.1} MB\n", sys.used_memory() / 1024));
        output.push_str(&format!("  Memory Total: {:.1} MB\n", sys.total_memory() / 1024));
        output.push_str(&format!("  Memory Usage: {:.1}%\n", 
            (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0));
            
        // Server metrics
        if self.is_server_running().await {
            if let Ok(metrics) = self.server_control.get_metrics().await {
                output.push_str("\n🔄 Server Metrics:\n");
                output.push_str(&format!("  Active Connections: {}\n", metrics.active_agents));
                output.push_str(&format!("  Error Count: {}\n", metrics.error_count));
                output.push_str(&format!("  Token Usage: {}\n", metrics.token_usage));
                output.push_str(&format!("  Token Cost: ${:.2}\n", metrics.token_cost));
            }
        }
        
        Ok(output)
    }

    async fn handle_monitor_health(&self) -> Result<String, NexaError> {
        let mut output = String::new();
        output.push_str("\nSystem Health Status:\n");
        
        if self.is_server_running().await {
            if let Ok(health) = self.server_control.check_health().await {
                output.push_str(&format!("\n🏥 Health Check:\n"));
                output.push_str(&format!("  Status: {}\n", 
                    if health.is_healthy { "✅ Healthy" } else { "❌ Unhealthy" }));
                output.push_str(&format!("  Message: {}\n", health.message));
                
                // Get detailed metrics for health analysis
                if let Ok(metrics) = self.server_control.get_metrics().await {
                    output.push_str("\n📊 Health Metrics:\n");
                    
                    // CPU health
                    let cpu_status = if metrics.cpu_usage < 70.0 { "✅ Good" }
                        else if metrics.cpu_usage < 85.0 { "⚠️ Warning" }
                        else { "❌ Critical" };
                    output.push_str(&format!("  CPU Load: {} ({:.1}%)\n", cpu_status, metrics.cpu_usage));
                    
                    // Memory health
                    let memory_used_pct = (metrics.memory_used as f64 / metrics.memory_allocated as f64) * 100.0;
                    let memory_status = if memory_used_pct < 75.0 { "✅ Good" }
                        else if memory_used_pct < 90.0 { "⚠️ Warning" }
                        else { "❌ Critical" };
                    output.push_str(&format!("  Memory Usage: {} ({:.1}%)\n", memory_status, memory_used_pct));
                    
                    // Connection health
                    let conn_status = if metrics.active_agents < 100 { "✅ Good" }
                        else if metrics.active_agents < 200 { "⚠️ Warning" }
                        else { "❌ Critical" };
                    output.push_str(&format!("  Connections: {} ({})\n", conn_status, metrics.active_agents));
                }
            }
        } else {
            output.push_str("\n❌ Server is not running\n");
        }
        
        Ok(output)
    }

    async fn handle_monitor_logs(&self, follow: bool) -> Result<(), NexaError> {
        let log_file = self.pid_file.with_extension("log");
        
        if follow {
            // Use tail -f equivalent for following logs
            use tokio::process::Command;
            
            Command::new("tail")
                .arg("-f")
                .arg(&log_file)
                .spawn()?
                .wait()
                .await?;
        } else {
            // Read and display last 50 lines
            if let Ok(content) = tokio::fs::read_to_string(&log_file).await {
                let lines: Vec<&str> = content.lines().collect();
                let start = if lines.len() > 50 { lines.len() - 50 } else { 0 };
                
                for line in &lines[start..] {
                    println!("{}", line);
                }
            } else {
                println!("No logs found or log file is not accessible");
            }
        }
        
        Ok(())
    }

    async fn handle_config_show(&self) -> Result<String, NexaError> {
        let config = Config::load(&Config::get_config_path())?;
        let mut output = String::new();
        
        output.push_str("\n🔧 Current Configuration:\n");
        
        // Server config
        output.push_str("\n📡 Server Settings:\n");
        output.push_str(&format!("  Host: {}\n", config.server.host));
        output.push_str(&format!("  Port: {}\n", config.server.port));
        output.push_str(&format!("  Max Connections: {}\n", config.server.max_connections));
        output.push_str(&format!("  Connection Timeout: {}s\n", config.server.connection_timeout));
        
        // Monitoring config
        output.push_str("\n📊 Monitoring Settings:\n");
        output.push_str(&format!("  CPU Threshold: {:.1}%\n", config.monitoring.cpu_threshold));
        output.push_str(&format!("  Memory Threshold: {:.1}%\n", config.monitoring.memory_threshold));
        output.push_str(&format!("  Health Check Interval: {}s\n", config.monitoring.health_check_interval));
        output.push_str(&format!("  Detailed Metrics: {}\n", config.monitoring.detailed_metrics));
        
        // Logging config
        output.push_str("\n📝 Logging Settings:\n");
        output.push_str(&format!("  Level: {}\n", config.logging.level));
        output.push_str(&format!("  File: {}\n", config.logging.file));
        output.push_str(&format!("  Max Size: {}MB\n", config.logging.max_size));
        output.push_str(&format!("  Files to Keep: {}\n", config.logging.files_to_keep));
        
        Ok(output)
    }

    async fn handle_config_set(&self, key: &str, value: &str) -> Result<(), NexaError> {
        let config_path = Config::get_config_path();
        let mut config = Config::load(&config_path)?;
        
        // Parse the key path (e.g., "server.host" or "monitoring.cpu_threshold")
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            return Err(NexaError::config("Invalid key format. Use section.key (e.g., server.host)"));
        }
        
        match (parts[0], parts[1]) {
            // Server settings
            ("server", "host") => config.server.host = value.to_string(),
            ("server", "port") => config.server.port = value.parse()
                .map_err(|_| NexaError::config("Invalid port number"))?,
            ("server", "max_connections") => config.server.max_connections = value.parse()
                .map_err(|_| NexaError::config("Invalid max connections value"))?,
            ("server", "connection_timeout") => config.server.connection_timeout = value.parse()
                .map_err(|_| NexaError::config("Invalid connection timeout value"))?,
            
            // Monitoring settings
            ("monitoring", "cpu_threshold") => config.monitoring.cpu_threshold = value.parse()
                .map_err(|_| NexaError::config("Invalid CPU threshold value"))?,
            ("monitoring", "memory_threshold") => config.monitoring.memory_threshold = value.parse()
                .map_err(|_| NexaError::config("Invalid memory threshold value"))?,
            ("monitoring", "health_check_interval") => config.monitoring.health_check_interval = value.parse()
                .map_err(|_| NexaError::config("Invalid health check interval value"))?,
            ("monitoring", "detailed_metrics") => config.monitoring.detailed_metrics = value.parse()
                .map_err(|_| NexaError::config("Invalid detailed metrics value (use true/false)"))?,
            
            // Logging settings
            ("logging", "level") => {
                match value.to_lowercase().as_str() {
                    "trace" | "debug" | "info" | "warn" | "error" => config.logging.level = value.to_lowercase(),
                    _ => return Err(NexaError::config("Invalid log level (use trace/debug/info/warn/error)")),
                }
            }
            ("logging", "file") => config.logging.file = value.to_string(),
            ("logging", "max_size") => config.logging.max_size = value.parse()
                .map_err(|_| NexaError::config("Invalid max log size value"))?,
            ("logging", "files_to_keep") => config.logging.files_to_keep = value.parse()
                .map_err(|_| NexaError::config("Invalid files to keep value"))?,
            
            _ => return Err(NexaError::config("Invalid configuration key")),
        }
        
        // Save updated configuration
        config.save(&config_path)?;
        info!("Configuration updated successfully");
        
        Ok(())
    }

    async fn handle_config_reset(&self) -> Result<(), NexaError> {
        let config_path = Config::get_config_path();
        let config = Config::reset();
        config.save(&config_path)?;
        info!("Configuration reset to defaults");
        Ok(())
    }
}

impl Clone for CliController {
    fn clone(&self) -> Self {
        Self {
            server_control: self.server_control.clone(),
            pid_file: self.pid_file.clone(),
            state_file: self.state_file.clone(),
            socket_path: self.socket_path.clone(),
        }
    }
}

impl CliController {
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let cli = Cli::parse();

        match cli.command {
            Commands::Start { port, host } => {
                info!("Starting Nexa Core server on {}:{}", host, port);
                // Create a basic config string with the port and host
                let config = format!("{}:{}", host, port);
                self.handle_start(&Some(config)).await?;
                Ok(())
            }
            Commands::Stop => {
                info!("Stopping Nexa Core server");
                self.handle_stop().await?;
                Ok(())
            }
            Commands::Status => {
                info!("Checking Nexa Core server status");
                let status = self.handle_status().await?;
                println!("{}", status);
                Ok(())
            }
            Commands::Config { cmd } => {
                match cmd {
                    ConfigCommands::Show => {
                        info!("Showing current configuration");
                        let config = self.handle_config_show().await?;
                        println!("{}", config);
                        Ok(())
                    }
                    ConfigCommands::Set { key, value } => {
                        info!("Setting configuration value");
                        self.handle_config_set(&key, &value).await?;
                        Ok(())
                    }
                    ConfigCommands::Reset => {
                        info!("Resetting configuration to defaults");
                        self.handle_config_reset().await?;
                        Ok(())
                    }
                }
            }
            Commands::Cluster { cmd } => {
                match cmd {
                    ClusterCommands::Status => {
                        info!("Showing cluster status");
                        // Implementation needed
                        Ok(())
                    }
                    ClusterCommands::Join { address } => {
                        info!("Joining cluster with address: {}", address);
                        // Implementation needed
                        Ok(())
                    }
                    ClusterCommands::Leave => {
                        info!("Leaving cluster gracefully");
                        // Implementation needed
                        Ok(())
                    }
                    ClusterCommands::Nodes => {
                        info!("Listing all cluster nodes");
                        // Implementation needed
                        Ok(())
                    }
                }
            }
            Commands::Monitor { cmd } => {
                match cmd {
                    MonitorCommands::Metrics => {
                        info!("Showing detailed metrics");
                        let metrics = self.handle_monitor_metrics().await?;
                        println!("{}", metrics);
                        Ok(())
                    }
                    MonitorCommands::Health => {
                        info!("Showing health status");
                        let health = self.handle_monitor_health().await?;
                        println!("{}", health);
                        Ok(())
                    }
                    MonitorCommands::Logs { follow } => {
                        info!("Viewing or following logs");
                        self.handle_monitor_logs(follow).await?;
                        Ok(())
                    }
                }
            }
            Commands::Connections { cmd } => {
                match cmd {
                    ConnectionCommands::List => {
                        info!("Listing active connections");
                        // Implementation needed
                        Ok(())
                    }
                    ConnectionCommands::Limit { max } => {
                        info!("Setting connection limits");
                        // Implementation needed
                        Ok(())
                    }
                    ConnectionCommands::Disconnect { target } => {
                        info!("Disconnecting client(s)");
                        // Implementation needed
                        Ok(())
                    }
                }
            }
        }
    }
}

