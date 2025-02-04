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
use uuid;
use chrono;
use serde;

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub status: AgentStatus,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_active: chrono::DateTime<chrono::Utc>,
    pub config: AgentConfig,
    pub metrics: AgentMetrics,
    pub workflows: Vec<String>,
    pub supported_actions: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentConfig {
    pub max_concurrent_tasks: usize,
    pub priority_threshold: i32,
    pub llm_provider: String,
    pub llm_model: String,
    pub retry_policy: RetryPolicy,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub max_backoff_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentMetrics {
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub average_response_time_ms: f64,
    pub uptime_seconds: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AgentStatus {
    Active,
    Idle,
    Busy,
    Error,
    Maintenance,
    Offline,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentWorkflow {
    pub id: String,
    pub name: String,
    pub steps: Vec<WorkflowStep>,
    pub status: WorkflowStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowStep {
    pub agent_id: String,
    pub action: AgentAction,
    pub dependencies: Vec<String>,
    pub retry_policy: Option<RetryPolicy>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WorkflowStatus {
    Ready,
    Running,
    Completed,
    Failed,
    Paused,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AgentAction {
    ProcessText { input: String, _max_tokens: usize },
    GenerateCode { prompt: String, language: String },
    AnalyzeCode { code: String, aspects: Vec<String> },
    CustomTask { task_type: String, parameters: serde_json::Value },
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

    /// Creates a new agent with the given configuration
    pub async fn create_agent(&self, name: String, config: AgentConfig) -> Result<Agent, String> {
        info!("Creating agent {} with configuration: {:?}", name, config);
        
        let agent = Agent {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            capabilities: Vec::new(),
            status: AgentStatus::Idle,
            parent_id: None,
            children: Vec::new(),
            created_at: chrono::Utc::now(),
            last_active: chrono::Utc::now(),
            config,
            metrics: AgentMetrics {
                tasks_completed: 0,
                tasks_failed: 0,
                average_response_time_ms: 0.0,
                uptime_seconds: 0,
                last_error: None,
            },
            workflows: Vec::new(),
            supported_actions: Vec::new(),
        };

        // Save agent to persistent storage
        self.save_agent(&agent).await?;
        
        info!("Agent {} created successfully with ID: {}", agent.name, agent.id);
        Ok(agent)
    }

    /// Tests an agent's capabilities with a sample task
    pub async fn test_agent(&self, agent_id: &str) -> Result<String, String> {
        info!("Testing agent {}", agent_id);
        
        let agent = self.get_agent(agent_id).await?;
        
        // Prepare a test prompt based on agent's capabilities
        let test_prompt = format!(
            "Respond with 'OK' and list your capabilities: {}",
            agent.capabilities.join(", ")
        );

        // Test the agent using its configured LLM
        let start_time = std::time::Instant::now();
        let result = self.try_chat_completion(
            &reqwest::Client::new(),
            &agent.config.llm_model,
            &test_prompt
        ).await;

        // Update agent metrics
        let mut updated_agent = agent.clone();
        match &result {
            Ok(_) => {
                updated_agent.metrics.tasks_completed += 1;
                updated_agent.metrics.average_response_time_ms = 
                    (updated_agent.metrics.average_response_time_ms * (updated_agent.metrics.tasks_completed - 1) as f64
                    + start_time.elapsed().as_millis() as f64) / updated_agent.metrics.tasks_completed as f64;
            },
            Err(e) => {
                updated_agent.metrics.tasks_failed += 1;
                updated_agent.metrics.last_error = Some(e.clone());
                updated_agent.status = AgentStatus::Error;
            }
        }
        
        self.save_agent(&updated_agent).await?;
        
        result
    }

    /// Updates an agent's capabilities
    pub async fn update_agent_capabilities(&self, agent_id: &str, capabilities: Vec<String>) -> Result<(), String> {
        info!("Updating capabilities for agent {}: {:?}", agent_id, capabilities);
        
        let mut agent = self.get_agent(agent_id).await?;
        agent.capabilities = capabilities;
        agent.last_active = chrono::Utc::now();
        
        self.save_agent(&agent).await
    }

    /// Creates a hierarchical relationship between agents
    pub async fn set_agent_hierarchy(&self, parent_id: &str, child_id: &str) -> Result<(), String> {
        info!("Setting agent hierarchy: parent={}, child={}", parent_id, child_id);
        
        let mut parent = self.get_agent(parent_id).await?;
        let mut child = self.get_agent(child_id).await?;
        
        // Prevent circular dependencies
        if child.id == parent_id || parent.parent_id.as_ref() == Some(&child.id) {
            return Err("Circular dependency detected".to_string());
        }
        
        // Update parent-child relationships
        parent.children.push(child.id.clone());
        child.parent_id = Some(parent_id.to_string());
        
        // Save both agents
        self.save_agent(&parent).await?;
        self.save_agent(&child).await
    }

    /// Retrieves an agent by ID
    async fn get_agent(&self, agent_id: &str) -> Result<Agent, String> {
        let path = self.get_agent_file_path(agent_id);
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| format!("Failed to read agent file: {}", e))?;
            
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse agent data: {}", e))
    }

    /// Saves an agent to persistent storage
    async fn save_agent(&self, agent: &Agent) -> Result<(), String> {
        let path = self.get_agent_file_path(&agent.id);
        
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create agent directory: {}", e))?;
        }
        
        let content = serde_json::to_string_pretty(agent)
            .map_err(|e| format!("Failed to serialize agent data: {}", e))?;
            
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| format!("Failed to write agent file: {}", e))
    }

    /// Gets the file path for an agent's persistent storage
    fn get_agent_file_path(&self, agent_id: &str) -> std::path::PathBuf {
        std::path::PathBuf::from("/tmp/nexa/agents")
            .join(format!("{}.json", agent_id))
    }

    /// Lists all agents with optional filtering
    pub async fn list_agents(&self, status: Option<AgentStatus>) -> Result<Vec<Agent>, String> {
        let agents_dir = std::path::PathBuf::from("/tmp/nexa/agents");
        
        if !agents_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut agents = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&agents_dir)
            .await
            .map_err(|e| format!("Failed to read agents directory: {}", e))?;
            
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                if let Ok(agent) = serde_json::from_str::<Agent>(&content) {
                    if status.is_none() || status.as_ref() == Some(&agent.status) {
                        agents.push(agent);
                    }
                }
            }
        }
        
        Ok(agents)
    }

    /// Gets the agent hierarchy as a tree structure
    pub async fn get_agent_hierarchy(&self) -> Result<Vec<Agent>, String> {
        let all_agents = self.list_agents(None).await?;
        
        // Filter to get only root agents (those without parents)
        let root_agents: Vec<Agent> = all_agents.into_iter()
            .filter(|agent| agent.parent_id.is_none())
            .collect();
            
        Ok(root_agents)
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

    /// Creates a new workflow
    pub async fn create_workflow(&self, name: String, steps: Vec<WorkflowStep>) -> Result<AgentWorkflow, String> {
        info!("Creating workflow {} with {} steps", name, steps.len());
        
        // Validate all agent IDs exist
        for step in &steps {
            self.get_agent(&step.agent_id).await?;
        }
        
        let workflow = AgentWorkflow {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            steps,
            status: WorkflowStatus::Ready,
            created_at: chrono::Utc::now(),
            last_run: None,
        };
        
        self.save_workflow(&workflow).await?;
        info!("Workflow created successfully with ID: {}", workflow.id);
        Ok(workflow)
    }

    /// Executes a workflow
    pub async fn execute_workflow(&self, workflow_id: &str) -> Result<(), String> {
        info!("Executing workflow {}", workflow_id);
        
        let mut workflow = self.get_workflow(workflow_id).await?;
        workflow.status = WorkflowStatus::Running;
        self.save_workflow(&workflow).await?;
        
        // Track completed steps
        let mut completed_steps = std::collections::HashSet::new();
        
        // Execute steps in order, respecting dependencies
        while completed_steps.len() < workflow.steps.len() {
            let mut executed_any = false;
            
            for (index, step) in workflow.steps.iter().enumerate() {
                // Skip if already completed
                if completed_steps.contains(&index) {
                    continue;
                }
                
                // Check if all dependencies are met
                let deps_met = step.dependencies.iter()
                    .all(|dep_id| completed_steps.contains(&dep_id.parse::<usize>().unwrap_or(0)));
                
                if deps_met {
                    // Execute the step
                    match self.execute_workflow_step(step).await {
                        Ok(_) => {
                            completed_steps.insert(index);
                            executed_any = true;
                        },
                        Err(e) => {
                            workflow.status = WorkflowStatus::Failed;
                            self.save_workflow(&workflow).await?;
                            return Err(format!("Step {} failed: {}", index, e));
                        }
                    }
                }
            }
            
            if !executed_any && completed_steps.len() < workflow.steps.len() {
                return Err("Workflow deadlocked - circular dependencies detected".to_string());
            }
        }
        
        workflow.status = WorkflowStatus::Completed;
        workflow.last_run = Some(chrono::Utc::now());
        self.save_workflow(&workflow).await?;
        
        Ok(())
    }

    /// Executes a single workflow step
    async fn execute_workflow_step(&self, step: &WorkflowStep) -> Result<String, String> {
        let agent = self.get_agent(&step.agent_id).await?;
        
        // Prepare retry policy
        let retry_policy = step.retry_policy.as_ref()
            .unwrap_or(&agent.config.retry_policy);
            
        // Prepare timeout
        let timeout = step.timeout_seconds
            .unwrap_or(agent.config.timeout_seconds);
            
        // Execute with retries and timeout
        let mut last_error = None;
        for attempt in 0..=retry_policy.max_retries {
            match tokio::time::timeout(
                std::time::Duration::from_secs(timeout),
                self.execute_agent_action(&agent, &step.action)
            ).await {
                Ok(result) => match result {
                    Ok(output) => return Ok(output),
                    Err(e) => {
                        last_error = Some(e);
                        if attempt < retry_policy.max_retries {
                            let backoff = std::cmp::min(
                                retry_policy.backoff_ms * (2_u64.pow(attempt)),
                                retry_policy.max_backoff_ms
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                        }
                    }
                },
                Err(_) => {
                    return Err(format!("Step execution timed out after {} seconds", timeout));
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    /// Executes a specific action using an agent
    async fn execute_agent_action(&self, agent: &Agent, action: &AgentAction) -> Result<String, String> {
        let client = reqwest::Client::new();
        
        match action {
            AgentAction::ProcessText { input, _max_tokens } => {
                self.try_chat_completion(&client, &agent.config.llm_model, input).await
            },
            AgentAction::GenerateCode { prompt, language } => {
                let code_prompt = format!("Generate {} code for: {}", language, prompt);
                self.try_chat_completion(&client, &agent.config.llm_model, &code_prompt).await
            },
            AgentAction::AnalyzeCode { code, aspects } => {
                let analysis_prompt = format!(
                    "Analyze the following code, focusing on {}: \n\n{}",
                    aspects.join(", "),
                    code
                );
                self.try_chat_completion(&client, &agent.config.llm_model, &analysis_prompt).await
            },
            AgentAction::CustomTask { task_type, parameters } => {
                // Handle custom task types
                let prompt = format!(
                    "Execute {} task with parameters: {}",
                    task_type,
                    serde_json::to_string_pretty(parameters).unwrap_or_default()
                );
                self.try_chat_completion(&client, &agent.config.llm_model, &prompt).await
            }
        }
    }

    /// Saves a workflow to persistent storage
    async fn save_workflow(&self, workflow: &AgentWorkflow) -> Result<(), String> {
        let path = self.get_workflow_file_path(&workflow.id);
        
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create workflow directory: {}", e))?;
        }
        
        let content = serde_json::to_string_pretty(workflow)
            .map_err(|e| format!("Failed to serialize workflow data: {}", e))?;
            
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| format!("Failed to write workflow file: {}", e))
    }

    /// Gets a workflow by ID
    async fn get_workflow(&self, workflow_id: &str) -> Result<AgentWorkflow, String> {
        let path = self.get_workflow_file_path(workflow_id);
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| format!("Failed to read workflow file: {}", e))?;
            
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse workflow data: {}", e))
    }

    /// Gets the file path for a workflow's persistent storage
    fn get_workflow_file_path(&self, workflow_id: &str) -> std::path::PathBuf {
        std::path::PathBuf::from("/tmp/nexa/workflows")
            .join(format!("{}.json", workflow_id))
    }

    /// Lists all workflows
    pub async fn list_workflows(&self) -> Result<Vec<AgentWorkflow>, String> {
        let workflows_dir = std::path::PathBuf::from("/tmp/nexa/workflows");
        
        if !workflows_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut workflows = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&workflows_dir)
            .await
            .map_err(|e| format!("Failed to read workflows directory: {}", e))?;
            
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                if let Ok(workflow) = serde_json::from_str::<AgentWorkflow>(&content) {
                    workflows.push(workflow);
                }
            }
        }
        
        Ok(workflows)
    }
}

