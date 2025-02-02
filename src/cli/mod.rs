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

    fn is_server_running(&self) -> bool {
        if let Ok(pid_str) = fs::read_to_string(&self.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                return unsafe { libc::kill(pid, 0) == 0 };
            }
        }
        false
    }

    async fn handle_start(&self) -> Result<(), NexaError> {
        if self.is_server_running() {
            println!("Server is already running");
            return Ok(());
        }

        // Write PID file
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

        info!("Starting Nexa Core server on 0.0.0.0:8080");
        self.server.start(None).await?;

        Ok(())
    }

    async fn handle_stop(&self) -> Result<(), NexaError> {
        if !self.is_server_running() {
            println!("Server is not running");
            return Ok(());
        }

        let pid_str = fs::read_to_string(&self.pid_file)
            .map_err(|e| NexaError::system(format!("Failed to read PID file: {}", e)))?;
        let pid = pid_str.trim().parse::<i32>()
            .map_err(|e| NexaError::system(format!("Invalid PID in file: {}", e)))?;

        // Send SIGTERM
        if let Err(e) = signal::kill(Pid::from_raw(pid), signal::Signal::SIGTERM) {
            error!("Failed to send SIGTERM to process {}: {}", pid, e);
        }

        // Wait for server to stop
        for _ in 0..10 {
            if !self.is_server_running() {
                if let Err(e) = fs::remove_file(&self.pid_file) {
                    eprintln!("Warning: Failed to remove PID file: {}", e);
                }
                println!("Server stopped successfully");
                return Ok(());
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        println!("Warning: Server did not stop gracefully");
        Ok(())
    }

    async fn handle_status(&self) -> Result<(), NexaError> {
        info!("Checking Nexa Core server status");
        
        let mut status = String::from("\nSystem Status:\n\n");

        // Get resource usage
        let mut sys_info = sysinfo::System::new_all();
        sys_info.refresh_all();
        let cpu_usage = sys_info.global_cpu_info().cpu_usage();
        let memory_usage = sys_info.used_memory() as f32 / sys_info.total_memory() as f32 * 100.0;
        
        status.push_str(&format!("Resource Usage:\n  CPU: {:.1}%\n  Memory: {:.1}%\n\n", 
            cpu_usage, memory_usage));

        let is_running = self.is_server_running();
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
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let handler = CliHandler::new();

    match cli.command {
        Commands::Start => handler.handle_start().await?,
        Commands::Stop => handler.handle_stop().await?,
        Commands::Status => handler.handle_status().await?,
    }

    Ok(())
}

