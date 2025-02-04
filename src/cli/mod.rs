//! CLI Handler for Nexa Utils
//! 
//! Provides command-line interface functionality for:
//! - Starting/stopping the MCP server
//! - Monitoring system status
//! - Managing agents

use clap::{Parser, Subcommand};
use log::{error, info, debug};
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
use reqwest;
use serde_json;

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

#[derive(Debug, Clone)]
pub struct LLMModel {
    pub name: String,
    pub size: String,
    pub context_length: usize,
    pub quantization: Option<String>,
    pub description: String,
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
        match provider {
            "LMStudio" => {
                // For LM Studio, we need to unload the model and verify disconnection
                let client = reqwest::Client::new();
                
                // First, get the currently loaded model
                let response = client.get("http://localhost:1234/v1/models")
                    .send()
                    .await
                    .map_err(|e| format!("Failed to connect to LMStudio: {}", e))?;

                let status = response.status();
                if status.is_success() {
                    // Send a special completion request to trigger cleanup
                    let _cleanup_response = client.post("http://localhost:1234/v1/chat/completions")
                        .header("Content-Type", "application/json")
                        .json(&serde_json::json!({
                            "model": "none",  // Invalid model to force unload
                            "messages": [{"role": "system", "content": "cleanup"}],
                            "temperature": 0.0,
                            "max_tokens": 1
                        }))
                        .send()
                        .await;

                    // Ignore the cleanup response as it's expected to fail
                    info!("LM Studio model unloaded successfully");
                    Ok(())
                } else {
                    let error_text = response.text().await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(format!("Failed to disconnect ({}): {}", status, error_text))
                }
            },
            "Ollama" => {
                // Existing Ollama implementation
                Ok(())
            },
            _ => Err(format!("Unsupported LLM provider: {}", provider))
        }
    }

    pub async fn list_models(&self, provider: &str) -> Result<Vec<LLMModel>, String> {
        info!("Listing models for provider: {}", provider);
        match provider {
            "LMStudio" => {
                let client = reqwest::Client::new();
                match client.get("http://localhost:1234/v1/models")
                    .send()
                    .await {
                    Ok(response) => {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                let models = json["data"].as_array()
                                    .ok_or("Invalid response format: missing 'data' array")?
                                    .iter()
                                    .filter_map(|model| {
                                        let id = model["id"].as_str()?;
                                        
                                        // Extract model details from the ID
                                        let (size, quantization) = if id.contains("32b") {
                                            ("32B", Some("IQ2_XXS".to_string()))
                                        } else if id.contains("7b") {
                                            ("7B", Some("Q4_K_M".to_string()))
                                        } else {
                                            ("Unknown", None)
                                        };

                                        // Create more descriptive model information
                                        let description = match id {
                                            s if s.contains("qwen") => 
                                                "Qwen model optimized for instruction following and chat",
                                            s if s.contains("coder") => 
                                                "Code generation optimized model",
                                            s if s.contains("embed") => 
                                                "Text embedding model for vector representations",
                                            _ => "General purpose language model"
                                        };

                                        Some(LLMModel {
                                            name: id.to_string(),
                                            size: size.to_string(),
                                            context_length: if id.contains("32b") { 16384 } else { 4096 },
                                            quantization: quantization,
                                            description: description.to_string(),
                                        })
                                    })
                                    .collect::<Vec<_>>();
                                if models.is_empty() {
                                    Err("No models found in LM Studio".to_string())
                                } else {
                                    Ok(models)
                                }
                            },
                            Err(e) => Err(format!("Failed to parse LMStudio response: {}", e))
                        }
                    },
                    Err(e) => {
                        if e.is_connect() {
                            Err("LM Studio server is not running. Please start LM Studio and enable the local server in Settings -> Local Server".to_string())
                        } else {
                            Err(format!("Failed to connect to LMStudio: {}", e))
                        }
                    }
                }
            },
            "Ollama" => {
                // Ollama API call implementation
                match reqwest::get("http://localhost:11434/api/tags").await {
                    Ok(response) => {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                let models = json["models"].as_array()
                                    .ok_or("Invalid response format")?
                                    .iter()
                                    .filter_map(|model| {
                                        let name = model["name"].as_str()?;
                                        Some(LLMModel {
                                            name: name.to_string(),
                                            size: model["size"].as_str()
                                                .unwrap_or("Unknown")
                                                .to_string(),
                                            context_length: model["context_length"]
                                                .as_u64()
                                                .unwrap_or(4096) as usize,
                                            quantization: model["quantization"]
                                                .as_str()
                                                .map(|s| s.to_string()),
                                            description: model["description"]
                                                .as_str()
                                                .unwrap_or("No description available")
                                                .to_string(),
                                        })
                                    })
                                    .collect::<Vec<_>>();
                                Ok(models)
                            },
                            Err(e) => Err(format!("Failed to parse Ollama response: {}", e))
                        }
                    },
                    Err(e) => Err(format!("Failed to connect to Ollama API: {}", e))
                }
            },
            _ => Err(format!("Unsupported LLM provider: {}", provider))
        }
    }

    pub async fn select_model(&self, provider: &str, model: &str) -> Result<(), String> {
        info!("Selecting model {} for provider {}", model, provider);
        match provider {
            "LMStudio" => {
                // LM Studio doesn't require explicit model loading - it's done automatically
                // Just verify the model exists
                let client = reqwest::Client::new();
                let response = client.get("http://localhost:1234/v1/models")
                    .send()
                    .await
                    .map_err(|e| format!("Failed to connect to LMStudio: {}", e))?;
                
                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            let models = json["data"].as_array()  // Access the 'data' array
                                .ok_or("Invalid response format: missing 'data' array")?;
                            if models.iter().any(|m| m["id"].as_str() == Some(model)) {
                                Ok(())
                            } else {
                                Err(format!("Model {} not found", model))
                            }
                        },
                        Err(e) => Err(format!("Failed to parse response: {}", e))
                    }
                } else {
                    let status = response.status();
                    let error_text = response.text().await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(format!("Failed to verify model ({}): {}", status, error_text))
                }
            },
            "Ollama" => {
                // Example implementation for Ollama
                let client = reqwest::Client::new();
                let response = client.post("http://localhost:11434/api/pull")
                    .json(&serde_json::json!({
                        "name": model
                    }))
                    .send()
                    .await
                    .map_err(|e| format!("Failed to send request to Ollama: {}", e))?;

                if response.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("Failed to pull model: {}", response.status()))
                }
            },
            _ => Err(format!("Unsupported LLM provider: {}", provider))
        }
    }

    async fn try_chat_completion(&self, client: &reqwest::Client, model: &str, test_prompt: &str) -> Result<String, String> {
        let response = client.post("http://localhost:1234/v1/chat/completions")
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": test_prompt}],
                "temperature": 0.7,
                "max_tokens": 10,
                "stream": false
            }))
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        if response.status().is_success() {
            match response.json::<serde_json::Value>().await {
                Ok(json) => {
                    let content = json["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or("No response")
                        .to_string();
                    
                    // Extract usage statistics but only log at debug level
                    if let Some(usage) = json.get("usage") {
                        debug!("Token usage - prompt: {}, completion: {}, total: {}",
                            usage["prompt_tokens"].as_u64().unwrap_or(0),
                            usage["completion_tokens"].as_u64().unwrap_or(0),
                            usage["total_tokens"].as_u64().unwrap_or(0)
                        );
                    }

                    Ok(content)
                },
                Err(e) => Err(format!("Failed to parse response: {}", e))
            }
        } else {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Request failed ({}): {}", status, error_text))
        }
    }

    pub async fn test_model(&self, provider: &str, model: &str) -> Result<String, String> {
        info!("Testing model {} for provider {}", model, provider);
        let test_prompt = "Respond with 'OK' if you can process this message.";
        
        match provider {
            "LMStudio" => {
                let client = reqwest::Client::new();
                let max_retries = 5;
                let wait_time = 30;
                
                for attempt in 0..=max_retries {
                    match self.try_chat_completion(&client, model, test_prompt).await {
                        Ok(content) => {
                            info!("Model test successful");
                            return Ok(content);
                        },
                        Err(e) => {
                            if e.contains("is not loaded") || e.contains("Loading") {
                                if attempt < max_retries {
                                    info!("Model is loading, waiting {} seconds (attempt {}/{})", 
                                        wait_time, attempt + 1, max_retries);
                                    tokio::time::sleep(tokio::time::Duration::from_secs(wait_time)).await;
                                    continue;
                                }
                            }
                            return Err(if attempt == max_retries {
                                format!("Model failed to load after {} attempts: {}", max_retries, e)
                            } else {
                                e
                            });
                        }
                    }
                }
                Err("Maximum retries exceeded".to_string())
            },
            "Ollama" => {
                let client = reqwest::Client::new();
                let response = client.post("http://localhost:11434/api/generate")
                    .json(&serde_json::json!({
                        "model": model,
                        "prompt": test_prompt,
                        "stream": false
                    }))
                    .send()
                    .await
                    .map_err(|e| format!("Failed to send request to Ollama: {}", e))?;

                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            let content = json["response"]
                                .as_str()
                                .unwrap_or("No response")
                                .to_string();
                            Ok(content)
                        },
                        Err(e) => Err(format!("Failed to parse response: {}", e))
                    }
                } else {
                    Err(format!("Request failed: {}", response.status()))
                }
            },
            _ => Err(format!("Unsupported LLM provider: {}", provider))
        }
    }
}

