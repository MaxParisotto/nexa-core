#![allow(dead_code, unused_imports, unused_variables)]

//! CLI Handler for Nexa Utils
//! 
//! Provides command-line interface functionality for:
//! - Starting/stopping the MCP server
//! - Monitoring system status
//! - Managing agents

use clap::{Parser, Subcommand};
use log::{info, error, debug};
use std::path::PathBuf;
use std::process;
use std::fs;
use nix::libc;
use reqwest;
use serde_json;
use uuid;
use chrono::Utc;
use crate::error::NexaError;
use crate::llm::system_helper::TaskPriority;
use crate::types::agent::{Agent, AgentStatus, AgentConfig, AgentMetrics};
use crate::types::workflow::{WorkflowStatus, WorkflowStep, AgentWorkflow, AgentAction, RetryPolicy};
use crate::server::{Server, ServerState};
use std::sync::Arc;
use crate::llm;
use std::time::Duration;
use sysinfo::System;
use serde::{Serialize, Deserialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LLMModel {
    pub name: String,
    pub provider: String,
    pub description: String,
    pub quantization: Option<String>,
}

impl LLMModel {
    pub fn new(name: String, provider: String, description: String, quantization: Option<String>) -> Self {
        Self {
            name,
            provider,
            description,
            quantization,
        }
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the Nexa server
    Start {
        /// Optional port number
        #[arg(short, long)]
        port: Option<u16>,
    },
    
    /// Stop the Nexa server
    Stop,
    
    /// Get server status
    Status,
    
    /// List all agents
    Agents {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
    },
    
    /// Create a new agent
    CreateAgent {
        /// Agent name
        #[arg(short, long)]
        name: String,
        /// LLM model to use
        #[arg(short, long)]
        model: Option<String>,
        /// LLM provider
        #[arg(short, long)]
        provider: Option<String>,
    },
    
    /// Stop an agent
    StopAgent {
        /// Agent ID
        #[arg(short, long)]
        id: String,
    },
    
    /// List available models
    Models {
        /// Provider to list models for
        #[arg(short, long)]
        provider: String,
    },
    
    /// Add LLM server
    AddServer {
        /// Provider name
        #[arg(short, long)]
        provider: String,
        /// Server URL
        #[arg(short, long)]
        url: String,
    },
    
    /// Remove LLM server
    RemoveServer {
        /// Provider name
        #[arg(short, long)]
        provider: String,
    },
    
    /// Create a new task
    CreateTask {
        /// Task description
        #[arg(short, long)]
        description: String,
        /// Task priority
        #[arg(short, long, default_value = "medium")]
        priority: String,
        /// Agent ID to assign task to
        #[arg(short, long)]
        agent_id: Option<String>,
    },
    
    /// List tasks
    Tasks,
    
    /// List workflows
    Workflows,
    
    /// Create a new workflow
    CreateWorkflow {
        /// Workflow name
        #[arg(short, long)]
        name: String,
        /// Workflow steps
        #[arg(short, long)]
        steps: Vec<String>,
    },
    
    /// Execute a workflow
    ExecuteWorkflow {
        /// Workflow ID
        #[arg(short, long)]
        id: String,
    },
}

#[derive(Debug, Clone)]
pub struct CliHandler {
    pid_file: PathBuf,
    state_file: PathBuf,
    socket_path: PathBuf,
    server: Server,
    llm_client: Arc<llm::LLMClient>,
    agents_dir: PathBuf,
    workflows_dir: PathBuf,
}

impl CliHandler {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let runtime_dir = PathBuf::from(home).join(".nexa");
        let pid_file = runtime_dir.join("nexa.pid");
        let state_file = runtime_dir.join("nexa.state");
        let socket_path = runtime_dir.join("nexa.sock");
        let agents_dir = runtime_dir.join("agents");
        let workflows_dir = runtime_dir.join("workflows");
        Self::with_paths(pid_file, state_file, socket_path)
    }

    pub fn with_paths(pid_file: PathBuf, state_file: PathBuf, socket_path: PathBuf) -> Self {
        let server = Server::new(pid_file.clone(), socket_path.clone());
        let llm_client = Arc::new(llm::LLMClient::new(llm::LLMConfig::default()).unwrap());
        let agents_dir = PathBuf::from("agents");
        let workflows_dir = PathBuf::from("workflows");
        Self {
            pid_file,
            state_file,
            socket_path,
            server,
            llm_client,
            agents_dir,
            workflows_dir,
        }
    }

    pub fn get_server(&self) -> &Server {
        &self.server
    }

    pub fn get_pid_file(&self) -> &PathBuf {
        &self.pid_file
    }

    pub fn is_server_running(&self) -> bool {
        // First check PID file
        if let Ok(pid_str) = fs::read_to_string(&self.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Check if process exists
                if unsafe { libc::kill(pid, 0) == 0 } {
                    // Process exists, check state file
                    if let Ok(state_str) = fs::read_to_string(&self.state_file) {
                        let state = state_str.trim();
                        return state == "Running" || state == "Starting";
                    }
                }
            }
        }
        
        // Clean up stale files
        let _ = fs::remove_file(&self.pid_file);
        let _ = fs::remove_file(&self.state_file);
        false
    }

    pub async fn save_server_state(&self, state: ServerState) -> Result<(), NexaError> {
        // Create parent directory if needed
        if let Some(parent) = self.state_file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| NexaError::Io(format!("Failed to create state directory: {}", e)))?;
        }
        
        // Write state file
        fs::write(&self.state_file, match state {
            ServerState::Running => "Running",
            ServerState::Starting => "Starting",
            ServerState::Stopping => "Stopping",
            ServerState::Stopped => "Stopped",
        }).map_err(|e| NexaError::Io(e.to_string()))?;

        // Handle PID file based on state
        match state {
            ServerState::Running => {
                fs::write(&self.pid_file, process::id().to_string())
                    .map_err(|e| NexaError::Io(e.to_string()))?;
            }
            ServerState::Stopped => {
                let _ = fs::remove_file(&self.pid_file);
                let _ = fs::remove_file(&self.state_file);
            }
            _ => {}
        }
        
        Ok(())
    }

    async fn load_server_state(&self) -> ServerState {
        if let Ok(state_str) = fs::read_to_string(&self.state_file) {
            match state_str.trim() {
                "Running" => ServerState::Running,
                "Starting" => ServerState::Starting,
                "Stopping" => ServerState::Stopping,
                _ => ServerState::Stopped,
            }
        } else {
            ServerState::Stopped
        }
    }

    pub async fn start(&self, port: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        // Create runtime directory if needed
        if let Some(parent) = self.state_file.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Check if server is already running
        if let Ok(state_str) = fs::read_to_string(&self.state_file) {
            if state_str.trim() == "Running" {
                return Err("Server is already running".into());
            }
        }
        
        // Start server
        let server = Server::new(
            self.pid_file.clone(),
            self.socket_path.clone()
        );
        
        server.start().await?;
        
        // Write initial state
        fs::write(&self.state_file, "Running")?;
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Check if server is running
        if let Ok(state_str) = fs::read_to_string(&self.state_file) {
            if state_str.trim() != "Running" {
                return Err("Server is not running".into());
            }
        } else {
            return Err("Server is not running".into());
        }
        
        // Stop server
        let server = Server::new(
            self.pid_file.clone(),
            self.socket_path.clone()
        );
        
        server.stop().await?;
        
        // Clean up state file
        let _ = fs::remove_file(&self.state_file);
        let _ = fs::remove_file(&self.pid_file);
        
        Ok(())
    }
    
    pub async fn status(&self) -> Result<(), Box<dyn std::error::Error>> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let runtime_dir = PathBuf::from(&home).join(".nexa");
        
        let mut sys = System::new_all();
        sys.refresh_all();
        
        let state = if let Ok(state_str) = fs::read_to_string(runtime_dir.join("nexa.state")) {
            state_str.trim().to_string()
        } else {
            "Stopped".to_string()
        };
        
        println!("\nServer Status: {} {}", 
            if state == "Running" { "🟢" } else { "🔴" },
            if state == "Running" { "Running" } else { "Stopped" }
        );
        println!("Server State: {:?}", state);
        println!("Resource Usage:");
        println!("  CPU: {:.1}%", sys.global_cpu_usage());
        println!("  Memory: {:.1}%", sys.used_memory() as f32 / sys.total_memory() as f32 * 100.0);
        
        if state != "Running" {
            println!("\nTo start the server, run: nexa start");
        }
        
        Ok(())
    }

    /// Creates a new agent with the given configuration
    pub async fn create_agent(&self, name: String, config: AgentConfig) -> Result<Agent, Box<dyn std::error::Error>> {
        info!("Creating agent {} with configuration: {:?}", name, config);
        
        let agent = Agent {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            capabilities: Vec::new(),
            status: AgentStatus::Offline,
            current_task: None,
            last_heartbeat: Utc::now(),
            parent_id: None,
            children: Vec::new(),
            last_active: Utc::now(),
            config,
            metrics: AgentMetrics::default(),
            workflows: Vec::new(),
            supported_actions: Vec::new(),
        };

        self.save_agent(&agent).await?;
        Ok(agent)
    }

    /// Tests an agent's capabilities with a sample task
    pub async fn test_agent(&self, agent_id: &str) -> Result<String, NexaError> {
        info!("Testing agent {}", agent_id);
        
        let agent = self.get_agent(agent_id).await?;
        
        // Prepare a test prompt based on agent's capabilities
        let test_prompt = format!(
            "Respond with 'OK' and list your capabilities: {}",
            agent.capabilities.join(", ")
        );

        // Test the agent using the LLM client
        let start_time = std::time::Instant::now();
        let result = self.llm_client.complete(&test_prompt).await;

        // Update agent metrics
        let mut updated_agent = agent.clone();
        match &result {
            Ok(_) => {
                updated_agent.metrics.tasks_completed += 1;
                updated_agent.metrics.cpu_usage = 
                    (updated_agent.metrics.cpu_usage * (updated_agent.metrics.tasks_completed - 1) as f64
                    + start_time.elapsed().as_secs() as f64) / updated_agent.metrics.tasks_completed as f64;
            },
            Err(_) => {
                updated_agent.metrics.tasks_failed += 1;
                updated_agent.status = AgentStatus::Offline;
            }
        }
        
        self.save_agent(&updated_agent).await?;
        result
    }

    /// Updates an agent's capabilities
    pub async fn update_agent_capabilities(&self, agent_id: &str, capabilities: Vec<String>) -> Result<(), NexaError> {
        let mut agent = self.get_agent(agent_id).await?;
        agent.capabilities = capabilities;
        agent.last_active = Utc::now();
        self.save_agent(&agent).await?;
        Ok(())
    }

    /// Creates a hierarchical relationship between agents
    pub async fn set_agent_hierarchy(&self, parent_id: &str, child_id: &str) -> Result<(), NexaError> {
        let mut parent = self.get_agent(parent_id).await?;
        let mut child = self.get_agent(child_id).await?;
        
        if parent.children.contains(&child_id.to_string()) {
            return Err(NexaError::Agent("Parent-child relationship already exists".to_string()));
        }
        
        parent.children.push(child_id.to_string());
        child.parent_id = Some(parent_id.to_string());
        
        self.save_agent(&parent).await?;
        self.save_agent(&child).await?;
        Ok(())
    }

    /// Retrieves an agent by ID
    async fn get_agent(&self, agent_id: &str) -> Result<Agent, NexaError> {
        let agent_file = self.agents_dir.join(format!("{}.json", agent_id));
        let content = fs::read_to_string(&agent_file)
            .map_err(|e| NexaError::Io(e.to_string()))?;
        serde_json::from_str(&content)
            .map_err(|e| NexaError::Json(e.to_string()))
    }

    /// Saves an agent to persistent storage
    async fn save_agent(&self, agent: &Agent) -> Result<(), NexaError> {
        let agent_file = self.agents_dir.join(format!("{}.json", agent.id));
        let content = serde_json::to_string_pretty(agent)
            .map_err(|e| NexaError::Json(e.to_string()))?;
        fs::write(&agent_file, content)
            .map_err(|e| NexaError::Io(e.to_string()))
    }

    /// Lists all agents with optional filtering
    pub async fn list_agents(&self, status: Option<AgentStatus>) -> Result<Vec<Agent>, Box<dyn std::error::Error>> {
        if !self.agents_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut agents = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&self.agents_dir)
            .await
            .map_err(|e| Box::new(e))?;
            
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
    pub async fn get_agent_hierarchy(&self) -> Result<Vec<Agent>, Box<dyn std::error::Error>> {
        let all_agents = self.list_agents(None).await?;
        
        // Filter to get only root agents (those without parents)
        let root_agents: Vec<Agent> = all_agents.into_iter()
            .filter(|agent| agent.parent_id.is_none())
            .collect();
            
        Ok(root_agents)
    }

    /// Creates a new task with the given description, priority and agent assignment.
    pub async fn create_task(&self, description: String, priority: TaskPriority, agent_id: Option<String>) -> Result<(), NexaError> {
        info!("Creating task: {} with priority {:?} for agent {}", description, priority, agent_id.as_deref().unwrap_or("none"));
        // TODO: Implement actual task creation
        Ok(())
    }

    /// Sets the maximum number of connections allowed.
    pub async fn set_max_connections(&self, max: u32) -> Result<(), NexaError> {
        info!("Setting max connections to {}", max);
        // TODO: Implement actual connection limit setting
        Ok(())
    }

    pub async fn add_llm_server(&self, provider: &str, url: &str) -> Result<(), NexaError> {
        info!("Adding LLM server: {} at {}", provider, url);
        // TODO: Add proper LLM server configuration
        Ok(())
    }

    pub async fn remove_llm_server(&self, provider: &str) -> Result<(), NexaError> {
        info!("Removing LLM server: {}", provider);
        // TODO: Remove LLM server configuration
        Ok(())
    }

    pub async fn connect_llm(&self, provider: &str) -> Result<(), NexaError> {
        info!("Connecting to LLM server: {}", provider);
        match provider {
            "openai" => {
                // Implementation for OpenAI
                Ok(())
            }
            "lmstudio" => {
                let client = reqwest::Client::new();
                let response = client.get("http://localhost:1234/v1/models")
                    .send()
                    .await
                    .map_err(|e| NexaError::System(e.to_string()))?;

                if response.status().is_success() {
                    let models: serde_json::Value = response.json().await
                        .map_err(|e| NexaError::Json(e.to_string()))?;
                    
                    if let Some(data) = models.get("data") {
                        if let Some(array) = data.as_array() {
                            let models = array.iter()
                                .filter_map(|m| m.get("id"))
                                .filter_map(|id| id.as_str())
                                .map(|s| s.to_string())
                                .collect::<Vec<_>>();
                            if models.is_empty() {
                                Err(NexaError::System("No models found in LM Studio".to_string()))
                            } else {
                                Ok(())
                            }
                        } else {
                            Err(NexaError::System("Invalid response format from LM Studio".to_string()))
                        }
                    } else {
                        Err(NexaError::System("Invalid response format from LM Studio".to_string()))
                    }
                } else {
                    let status = response.status();
                    let error_text = response.text().await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(NexaError::System(format!("Failed to connect ({}): {}", status, error_text)))
                }
            }
            _ => Err(NexaError::System(format!("Unsupported LLM provider: {}", provider)))
        }
    }

    pub async fn disconnect_llm(&self, provider: &str) -> Result<(), NexaError> {
        info!("Disconnecting from LLM server: {}", provider);
        match provider {
            "openai" => {
                // Implementation for OpenAI
                Ok(())
            }
            "lmstudio" => {
                let client = reqwest::Client::new();
                let response = client.post("http://localhost:1234/v1/disconnect")
                    .send()
                    .await
                    .map_err(|e| NexaError::LLMError(e.to_string()))?;

                if response.status().is_success() {
                    Ok(())
                } else {
                    let status = response.status();
                    let error_text = response.text().await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(NexaError::LLMError(format!("Failed to disconnect ({}): {}", status, error_text)))
                }
            }
            _ => Err(NexaError::LLMError(format!("Unsupported LLM provider: {}", provider)))
        }
    }

    pub async fn list_models(&self, provider: &str) -> Result<Vec<LLMModel>, Box<dyn std::error::Error>> {
        match provider {
            "lmstudio" => {
                let client = reqwest::Client::new();
                match client.get("http://localhost:1234/v1/models")
                    .send()
                    .await {
                    Ok(response) => {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                let models = json["data"]
                                    .as_array()
                                    .unwrap_or(&Vec::new())
                                    .iter()
                                    .map(|m| LLMModel {
                                        name: m["id"].as_str().unwrap_or("unknown").to_string(),
                                        provider: "LM Studio".to_string(),
                                        description: "LM Studio model".to_string(),
                                        quantization: None,
                                    })
                                    .collect::<Vec<_>>();
                                if models.is_empty() {
                                    Err(Box::new(NexaError::LLMError("No models found in LM Studio".to_string())))
                                } else {
                                    Ok(models)
                                }
                            },
                            Err(e) => Err(Box::new(NexaError::LLMError(format!("Failed to parse LMStudio response: {}", e))))
                        }
                    },
                    Err(e) => {
                        if e.is_connect() {
                            Err(Box::new(NexaError::LLMError("LM Studio server is not running. Please start LM Studio and enable the local server in Settings -> Local Server".to_string())))
                        } else {
                            Err(Box::new(NexaError::LLMError(format!("Failed to connect to LMStudio: {}", e))))
                        }
                    }
                }
            },
            "ollama" => {
                let client = reqwest::Client::new();
                match client.get("http://localhost:11434/api/tags")
                    .send()
                    .await {
                    Ok(response) => {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                let models = json["models"]
                                    .as_array()
                                    .unwrap_or(&Vec::new())
                                    .iter()
                                    .map(|m| LLMModel {
                                        name: m["name"].as_str().unwrap_or("unknown").to_string(),
                                        provider: "Ollama".to_string(),
                                        description: "Ollama model".to_string(),
                                        quantization: None,
                                    })
                                    .collect::<Vec<_>>();
                                Ok(models)
                            },
                            Err(e) => Err(Box::new(NexaError::LLMError(format!("Failed to parse Ollama response: {}", e))))
                        }
                    },
                    Err(e) => Err(Box::new(NexaError::LLMError(format!("Failed to connect to Ollama API: {}", e))))
                }
            },
            _ => Err(Box::new(NexaError::LLMError(format!("Unsupported LLM provider: {}", provider))))
        }
    }

    pub async fn select_model(&self, provider: &str, model: &str) -> Result<(), NexaError> {
        info!("Selecting model {} for provider {}", model, provider);
        match provider {
            "LMStudio" => {
                let client = reqwest::Client::new();
                let response = client.get("http://localhost:1234/v1/models")
                    .send()
                    .await?;
                
                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            let models = json["data"].as_array()
                                .ok_or_else(|| NexaError::LLMError("Invalid response format: missing 'data' array".to_string()))?;
                            if models.iter().any(|m| m["id"].as_str() == Some(model)) {
                                Ok(())
                            } else {
                                Err(NexaError::LLMError(format!("Model {} not found", model)))
                            }
                        },
                        Err(e) => Err(NexaError::LLMError(format!("Failed to parse response: {}", e)))
                    }
                } else {
                    let status = response.status();
                    let error_text = response.text().await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(NexaError::LLMError(format!("Failed to verify model ({}): {}", status, error_text)))
                }
            },
            "Ollama" => {
                let client = reqwest::Client::new();
                let response = client.post("http://localhost:11434/api/pull")
                    .json(&serde_json::json!({
                        "name": model
                    }))
                    .send()
                    .await?;

                if response.status().is_success() {
                    Ok(())
                } else {
                    Err(NexaError::LLMError(format!("Failed to pull model: {}", response.status())))
                }
            },
            _ => Err(NexaError::LLMError(format!("Unsupported LLM provider: {}", provider)))
        }
    }

    async fn try_chat_completion(&self, prompt: &str, model: &str) -> Result<String, NexaError> {
        let client = reqwest::Client::new();
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|e| NexaError::Config(e.to_string()))?;
        
        let response = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": prompt}]
            }))
            .send()
            .await
            .map_err(|e| NexaError::LLMError(e.to_string()))?;

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| NexaError::LLMError(e.to_string()))?;

        response_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| NexaError::LLMError("Failed to extract response content".to_string()))
    }

    pub async fn test_model(&self, model: &str, test_prompt: &str) -> Result<String, NexaError> {
        let client = reqwest::Client::new();
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|e| NexaError::Config(e.to_string()))?;
        
        let response = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": test_prompt}]
            }))
            .send()
            .await
            .map_err(|e| NexaError::LLMError(format!("Request failed: {}", e)))?;

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| NexaError::LLMError(format!("JSON parsing failed: {}", e)))?;

        response_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| NexaError::LLMError("Failed to extract response content".to_string()))
    }

    /// Creates a new workflow
    pub async fn create_workflow(&self, name: String, steps: Vec<WorkflowStep>) -> Result<AgentWorkflow, Box<dyn std::error::Error>> {
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
    pub async fn execute_workflow(&self, workflow_id: &str) -> Result<(), NexaError> {
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
                            return Err(NexaError::System(format!("Step {} failed: {}", index, e)));
                        }
                    }
                }
            }
            
            if !executed_any && completed_steps.len() < workflow.steps.len() {
                return Err(NexaError::System("Workflow deadlocked - circular dependencies detected".to_string()));
            }
        }
        
        workflow.status = WorkflowStatus::Completed;
        workflow.last_run = Some(chrono::Utc::now());
        self.save_workflow(&workflow).await?;
        
        Ok(())
    }

    /// Executes a single workflow step
    async fn execute_workflow_step(&self, step: &WorkflowStep) -> Result<String, NexaError> {
        let agent = self.get_agent(&step.agent_id).await?;
        
        // Execute with timeout
        let timeout = step.timeout_seconds.unwrap_or(60); // Default 60 second timeout
            
        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout),
            self.execute_agent_action(&agent, &step.action)
        ).await {
            Ok(result) => result.map_err(|e| NexaError::LLMError(e.to_string())),
            Err(_) => Err(NexaError::LLMError(format!("Step execution timed out after {} seconds", timeout)))
        }
    }

    /// Executes a specific action using an agent
    pub async fn execute_agent_action(&self, agent: &Agent, action: &AgentAction) -> Result<String, Box<dyn std::error::Error>> {
        match action {
            AgentAction::ProcessText { input, max_tokens: _ } => {
                Ok(self.try_chat_completion(input, &agent.config.llm_model).await?)
            },
            AgentAction::GenerateCode { prompt, language } => {
                let code_prompt = format!("Generate {} code for: {}", language, prompt);
                Ok(self.try_chat_completion(&code_prompt, &agent.config.llm_model).await?)
            },
            AgentAction::AnalyzeCode { code, aspects } => {
                let analysis_prompt = format!(
                    "Analyze this code focusing on these aspects: {}\n\nCode:\n{}", 
                    aspects.join(", "), 
                    code
                );
                Ok(self.try_chat_completion(&analysis_prompt, &agent.config.llm_model).await?)
            },
            AgentAction::CustomTask { task_type, parameters } => {
                // For testing purposes, just return a success message
                Ok(format!("Executed custom task '{}' with parameters: {}", task_type, parameters))
            }
        }
    }

    /// Saves a workflow to persistent storage
    async fn save_workflow(&self, workflow: &AgentWorkflow) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.get_workflow_file_path(&workflow.id);
        let content = serde_json::to_string_pretty(workflow)?;
        tokio::fs::write(&path, content).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    /// Gets a workflow by ID
    async fn get_workflow(&self, workflow_id: &str) -> Result<AgentWorkflow, Box<dyn std::error::Error>> {
        let path = self.get_workflow_file_path(workflow_id);
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            
        serde_json::from_str(&content).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    /// Gets the file path for a workflow's persistent storage
    fn get_workflow_file_path(&self, workflow_id: &str) -> std::path::PathBuf {
        self.workflows_dir.join(format!("{}.json", workflow_id))
    }

    /// Lists all workflows
    pub async fn list_workflows(&self) -> Result<Vec<AgentWorkflow>, Box<dyn std::error::Error>> {
        if !self.workflows_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut workflows = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&self.workflows_dir)
            .await
            .map_err(|e| Box::new(e))?;
            
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                if let Ok(workflow) = serde_json::from_str::<AgentWorkflow>(&content) {
                    workflows.push(workflow);
                }
            }
        }
        
        Ok(workflows)
    }

    pub async fn update_agent_config(&self, id: String, config: AgentConfig) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(mut agent) = self.get_agent(&id).await {
            agent.config = config;
            Ok(self.save_agent(&agent).await?)
        } else {
            Err(Box::new(NexaError::Agent(format!("Agent {} not found", id))))
        }
    }

    pub async fn stop_agent(&self, agent_id: &str) -> Result<(), NexaError> {
        let agent = self.get_agent(agent_id).await?;
        
        if agent.status == AgentStatus::Offline {
            return Err(NexaError::Agent("Agent is already stopped".to_string()));
        }
        
        let mut updated_agent = agent.clone();
        updated_agent.status = AgentStatus::Offline;
        updated_agent.last_active = Utc::now();
        
        self.save_agent(&updated_agent).await?;
        
        if let Some(parent_id) = &agent.parent_id {
            if let Ok(mut parent) = self.get_agent(parent_id).await {
                parent.children.retain(|id| id != agent_id);
                self.save_agent(&parent).await?;
            }
        }
        
        Ok(())
    }

    pub async fn get_agent_config(&self, agent_id: &str) -> Result<AgentConfig, Box<dyn std::error::Error>> {
        let agent = self.get_agent(agent_id).await?;
        Ok(agent.config)
    }

    pub async fn test_agent_with_prompt(&mut self, agent: &mut Agent, test_prompt: &str) -> Result<String, NexaError> {
        let result = self.llm_client.complete(test_prompt).await;
        
        match result {
            Ok(response) => {
                agent.metrics.tasks_completed += 1;
                agent.status = AgentStatus::Idle;
                agent.last_active = Utc::now();
                agent.last_heartbeat = Utc::now();
                Ok(response)
            }
            Err(e) => {
                agent.metrics.tasks_failed += 1;
                agent.status = AgentStatus::Offline;
                agent.last_heartbeat = Utc::now();
                Err(e)
            }
        }
    }

    pub fn create_new_agent(&self, id: String, name: String, capabilities: Vec<String>) -> Agent {
        Agent {
            id,
            name,
            capabilities,
            status: AgentStatus::Offline,
            current_task: None,
            last_heartbeat: Utc::now(),
            parent_id: None,
            children: Vec::new(),
            last_active: Utc::now(),
            config: AgentConfig::default(),
            metrics: AgentMetrics::default(),
            workflows: Vec::new(),
            supported_actions: Vec::new(),
        }
    }

    pub fn handle_error(&self, error: Box<dyn std::error::Error>) -> NexaError {
        NexaError::System(error.to_string())
    }

    /// Updates an agent's status
    pub async fn update_agent_status(&self, agent_id: &str, status: AgentStatus) -> Result<(), NexaError> {
        let mut agent = self.get_agent(agent_id).await?;
        agent.status = status;
        agent.last_active = Utc::now();
        self.save_agent(&agent).await
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_ms: 1000,
            max_backoff_ms: 10000,
        }
    }
}

