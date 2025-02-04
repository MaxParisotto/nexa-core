//! CLI Handler for Nexa Utils
//! 
//! Provides command-line interface functionality for:
//! - Starting/stopping the MCP server
//! - Monitoring system status
//! - Managing agents

use clap::{Parser, Subcommand};
use log::{error, info};
use crate::server::Server;
use std::path::PathBuf;
use crate::error::NexaError;
use sysinfo;
use std::process;
use std::fs;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use nix::libc;
use crate::gui::TaskPriority;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the server
    Start,
    /// Stop the server
    Stop,
    /// Get server status
    Status,
    /// Launch the GUI
    Gui,
}

pub struct CliHandler {
    pid_file: PathBuf,
    server: Server,
}

impl CliHandler {
    pub fn new() -> Self {
        let pid_file = PathBuf::from("/tmp/nexa.pid");
        let socket_path = PathBuf::from("/tmp/nexa.sock");
        Self::with_paths(pid_file, socket_path)
    }

    pub fn with_paths(pid_file: PathBuf, socket_path: PathBuf) -> Self {
        let server = Server::new(pid_file.clone(), socket_path);
        Self { pid_file, server }
    }

    pub fn get_server(&self) -> &Server {
        &self.server
    }

    pub fn is_server_running(&self) -> bool {
        if let Ok(pid_str) = fs::read_to_string(&self.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                return unsafe { libc::kill(pid, 0) == 0 };
            }
        }
        false
    }

    pub async fn start(&self, _addr: Option<&str>) -> Result<(), NexaError> {
        if self.is_server_running() {
            println!("Server is already running");
            return Ok(());
        }

        fs::write(&self.pid_file, process::id().to_string())
            .map_err(|e| NexaError::System(format!("Failed to write PID file: {}", e)))?;

        info!("Starting Nexa Core server");
        self.server.start().await?;

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), NexaError> {
        if !self.is_server_running() {
            println!("Server is not running");
            return Ok(());
        }

        if let Err(e) = self.server.stop().await {
            error!("Failed to stop server gracefully: {}", e);
        }

        if let Ok(pid_str) = fs::read_to_string(&self.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if let Err(e) = signal::kill(Pid::from_raw(pid), Signal::SIGTERM) {
                    error!("Failed to send SIGTERM to process {}: {}", pid, e);
                }

                let start = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(5);
                while start.elapsed() < timeout {
                    if !self.is_server_running() {
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }

                if self.is_server_running() {
                    error!("Server did not stop gracefully, sending SIGKILL");
                    let _ = signal::kill(Pid::from_raw(pid), Signal::SIGKILL);
                }
            }
        }

        if let Err(e) = fs::remove_file(&self.pid_file) {
            error!("Failed to remove PID file: {}", e);
        }

        println!("Server stopped");
        Ok(())
    }

    pub async fn status(&self) -> Result<(), NexaError> {
        info!("Checking Nexa Core server status");
        
        let mut status = String::from("\nSystem Status:\n\n");

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
                .map_err(|e| NexaError::System(format!("Failed to read PID file: {}", e)))?;
            status.push_str(&format!("Server is running on 0.0.0.0:8080\n"));
            status.push_str(&format!("PID: {}\n", pid.trim()));

            let metrics = self.server.get_metrics().await;
            status.push_str(&format!("\nServer Metrics:\n"));
            status.push_str(&format!("  Total Connections: {}\n", metrics.total_connections));
            status.push_str(&format!("  Active Connections: {}\n", metrics.active_connections));
            status.push_str(&format!("  Failed Connections: {}\n", metrics.failed_connections));
            if let Some(last_error) = metrics.last_error {
                status.push_str(&format!("  Last Error: {}\n", last_error));
            }
            status.push_str(&format!("  Uptime: {:?}\n", metrics.uptime));
        }

        println!("{}", status);
        Ok(())
    }

    /// Creates a new agent with the given name and capabilities.
    pub async fn create_agent(&self, name: String, capabilities: Vec<String>) -> Result<(), String> {
        info!("Creating agent {} with capabilities: {:?}", name, capabilities);
        // TODO: Implement actual agent creation
        Ok(())
    }

    /// Creates a new task with the given description, priority and agent assignment.
    pub async fn create_task(&self, description: String, priority: TaskPriority, agent_id: String) -> Result<(), String> {
        info!("Creating task: {} with priority {:?} for agent {}", description, priority, agent_id);
        // TODO: Implement actual task creation
        Ok(())
    }

    /// Sets the maximum number of connections allowed.
    pub async fn set_max_connections(&self, max: u32) -> Result<(), String> {
        info!("Setting max connections to {}", max);
        // TODO: Implement actual connection limit setting
        Ok(())
    }

    pub async fn add_llm_server(&self, provider: &str, address: &str) -> Result<(), String> {
        info!("Adding LLM server: {} at {}", provider, address);
        // TODO: Add proper LLM server configuration
        Ok(())
    }

    pub async fn remove_llm_server(&self, provider: &str) -> Result<(), String> {
        info!("Removing LLM server: {}", provider);
        // TODO: Remove LLM server configuration
        Ok(())
    }

    pub async fn connect_llm(&self, provider: &str) -> Result<(), String> {
        info!("Connecting to LLM server: {}", provider);
        match provider {
            "LMStudio" => {
                super::llm::LLMConnection::connect("LMStudio", "system".to_string()).await
                    .map_err(|e| e.to_string())
            },
            "Ollama" => {
                super::llm::LLMConnection::connect("Ollama", "system".to_string()).await
                    .map_err(|e| e.to_string())
            },
            _ => Err(format!("Unsupported LLM provider: {}", provider))
        }
    }

    pub async fn disconnect_llm(&self, provider: &str) -> Result<(), String> {
        info!("Disconnecting from LLM server: {}", provider);
        // TODO: Implement proper LLM disconnection
        Ok(())
    }
}

