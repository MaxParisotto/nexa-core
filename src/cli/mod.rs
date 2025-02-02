//! CLI Handler for Nexa Utils
//! 
//! Provides command-line interface functionality for:
//! - Starting/stopping the MCP server
//! - Monitoring system status
//! - Managing agents

use clap::{Parser, Subcommand};
use tracing::{error, info};
use crate::mcp::ServerControl;
use std::path::PathBuf;
use crate::error::NexaError;
use sysinfo;
use std::process;
use ctrlc;
use std::fs;
use nix::sys::signal;
use nix::unistd::Pid;
use nix::libc;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server
    Start,
    /// Stop the server
    Stop,
    /// Get server status
    Status,
}

pub struct CliHandler {
    pid_file: PathBuf,
    server: ServerControl,
}

impl CliHandler {
    pub fn new() -> Self {
        let pid_file = PathBuf::from("/tmp/nexa.pid");
        let server = ServerControl::new(
            pid_file.clone(),
            PathBuf::from("/tmp/nexa.sock"),
        );
        Self { pid_file, server }
    }

    pub fn new_with_paths(pid_file: PathBuf, socket_path: PathBuf) -> Self {
        let server = ServerControl::new(pid_file.clone(), socket_path);
        Self { pid_file, server }
    }

    pub async fn is_server_running(&self) -> bool {
        // First check if the PID file exists and process is running
        if let Ok(pid_str) = fs::read_to_string(&self.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if unsafe { libc::kill(pid, 0) } == 0 {
                    // PID exists and process is running, now check if server is bound
                    // Wait up to 1 second for the server to be ready
                    return tokio::time::timeout(
                        std::time::Duration::from_secs(1),
                        self.server.wait_for_ready()
                    ).await.unwrap_or(false);
                }
            }
        }
        false
    }

    pub async fn start(&self, addr: Option<&str>) -> Result<(), NexaError> {
        if self.is_server_running().await {
            println!("Server is already running");
            return Ok(());
        }

        // Write PID file first
        fs::create_dir_all(self.pid_file.parent().unwrap_or(&self.pid_file))
            .map_err(|e| NexaError::system(format!("Failed to create parent directory: {}", e)))?;

        fs::write(&self.pid_file, process::id().to_string())
            .map_err(|e| NexaError::system(format!("Failed to write PID file: {}", e)))?;

        // Setup signal handler for cleanup
        let pid_file = self.pid_file.clone();
        ctrlc::set_handler(move || {
            if let Err(e) = fs::remove_file(&pid_file) {
                eprintln!("Failed to remove PID file: {}", e);
            }
            process::exit(0);
        })?;

        info!("Starting Nexa Core server");
        
        // Start the server
        if let Err(e) = self.server.start(addr).await {
            // Clean up PID file on error
            let _ = fs::remove_file(&self.pid_file);
            return Err(e);
        }

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), NexaError> {
        if !self.is_server_running().await {
            println!("Server is not running");
            return Ok(());
        }

        // First try to stop the server gracefully
        if let Err(e) = self.server.stop().await {
            error!("Failed to stop server gracefully: {}", e);
        }

        // Read PID file and send signal
        if let Ok(pid_str) = fs::read_to_string(&self.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Send SIGTERM
                if let Err(e) = signal::kill(Pid::from_raw(pid), signal::Signal::SIGTERM) {
                    error!("Failed to send SIGTERM to process {}: {}", pid, e);
                }

                // Wait for server to stop with timeout
                let start = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(5);
                while start.elapsed() < timeout {
                    if !self.is_server_running().await {
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }

                // If server hasn't stopped, send SIGKILL
                if self.is_server_running().await {
                    error!("Server did not stop gracefully, sending SIGKILL");
                    let _ = signal::kill(Pid::from_raw(pid), signal::Signal::SIGKILL);
                }
            }
        }

        // Clean up PID file
        if let Err(e) = fs::remove_file(&self.pid_file) {
            error!("Failed to remove PID file: {}", e);
        }

        println!("Server stopped");
        Ok(())
    }

    pub async fn status(&self) -> Result<(), NexaError> {
        info!("Checking Nexa Core server status");
        
        let mut status = String::from("\nSystem Status:\n\n");

        // Get resource usage
        let mut sys_info = sysinfo::System::new_all();
        sys_info.refresh_all();
        let cpu_usage = sys_info.global_cpu_info().cpu_usage();
        let memory_usage = sys_info.used_memory() as f32 / sys_info.total_memory() as f32 * 100.0;
        
        status.push_str(&format!("Resource Usage:\n  CPU: {:.1}%\n  Memory: {:.1}%\n\n", 
            cpu_usage, memory_usage));

        let is_running = self.is_server_running().await;
        status.push_str(&format!("Server Status: {} {}\n\n",
            if is_running { "🟢" } else { "🔴" },
            if is_running { "Running" } else { "Stopped" }
        ));

        if !is_running {
            status.push_str("Server is not running. Start it with 'nexa start'\n");
        } else {
            let pid = fs::read_to_string(&self.pid_file)
                .map_err(|e| NexaError::system(format!("Failed to read PID file: {}", e)))?;
            status.push_str(&format!("Server is running on 0.0.0.0:8080\n"));
            status.push_str(&format!("PID: {}\n", pid.trim()));

            // Add server metrics if available
            if let Ok(metrics) = self.server.get_metrics().await {
                status.push_str(&format!("\nServer Metrics:\n"));
                status.push_str(&format!("  CPU Usage: {:.1}%\n", metrics.cpu_usage));
                status.push_str(&format!("  Memory Used: {:.1} MB\n", metrics.memory_used as f32 / 1024.0 / 1024.0));
                status.push_str(&format!("  Memory Available: {:.1} MB\n", metrics.memory_available as f32 / 1024.0 / 1024.0));
                status.push_str(&format!("  Token Usage: {}\n", metrics.token_usage));
            }
        }

        println!("{}", status);
        Ok(())
    }

    pub fn get_pid_file_path(&self) -> &PathBuf {
        &self.pid_file
    }
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let handler = CliHandler::new();

    match cli.command {
        Commands::Start => handler.start(None).await?,
        Commands::Stop => handler.stop().await?,
        Commands::Status => handler.status().await?,
    }

    Ok(())
}

