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
        }
    }
}

