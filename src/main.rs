use clap::Parser;
use log::{info, error};
use std::process;
use crate::cli::Cli;
use crate::cli::Commands;
use crate::cli::CliHandler;
use crate::types::agent::AgentConfig;
use crate::llm::system_helper::TaskPriority;
use crate::types::workflow::{WorkflowStep, AgentAction, RetryPolicy};

mod cli;
mod types;
mod llm;
mod error;
mod server;
mod api;
mod models;
mod monitoring;
mod memory;
mod tokens;
mod settings;
mod utils;
mod config;

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init();

    // Parse command line arguments
    let cli = Cli::parse();
    let handler = CliHandler::new();

    // Process commands
    match cli.command {
        Some(Commands::Start { port }) => {
            info!("Starting server...");
            let port_str = port.map(|p| p.to_string());
            if let Err(e) = handler.start(port_str.as_deref()).await {
                error!("Failed to start server: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::Stop) => {
            info!("Stopping server...");
            if let Err(e) = handler.stop().await {
                error!("Failed to stop server: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::Status) => {
            info!("Checking server status...");
            if let Err(e) = handler.status().await {
                error!("Failed to get server status: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::Agents { status }) => {
            let status_enum = status.map(|s| s.parse().unwrap_or_default());
            match handler.list_agents(status_enum).await {
                Ok(agents) => {
                    println!("\nAgents:");
                    for agent in agents {
                        println!("ID: {}", agent.id);
                        println!("Name: {}", agent.name);
                        println!("Status: {:?}", agent.status);
                        println!("Model: {}", agent.config.llm_model);
                        println!("Provider: {}", agent.config.llm_provider);
                        println!("---");
                    }
                }
                Err(e) => {
                    error!("Failed to list agents: {}", e);
                    process::exit(1);
                }
            }
        }
        Some(Commands::CreateAgent { name, model, provider }) => {
            let mut config = AgentConfig::default();
            if let Some(m) = model {
                config.llm_model = m;
            }
            if let Some(p) = provider {
                config.llm_provider = p;
            }
            
            match handler.create_agent(name, config).await {
                Ok(agent) => println!("Created agent: {}", agent.id),
                Err(e) => {
                    error!("Failed to create agent: {}", e);
                    process::exit(1);
                }
            }
        }
        Some(Commands::StopAgent { id }) => {
            if let Err(e) = handler.stop_agent(&id).await {
                error!("Failed to stop agent: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::Models { provider }) => {
            match handler.list_models(&provider).await {
                Ok(models) => {
                    println!("\nAvailable Models:");
                    for model in models {
                        println!("Name: {}", model.name);
                        println!("Provider: {}", model.provider);
                        println!("Description: {}", model.description);
                        if let Some(quant) = model.quantization {
                            println!("Quantization: {}", quant);
                        }
                        println!("---");
                    }
                }
                Err(e) => {
                    error!("Failed to list models: {}", e);
                    process::exit(1);
                }
            }
        }
        Some(Commands::AddServer { provider, url }) => {
            if let Err(e) = handler.add_llm_server(&provider, &url).await {
                error!("Failed to add server: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::RemoveServer { provider }) => {
            if let Err(e) = handler.remove_llm_server(&provider).await {
                error!("Failed to remove server: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::CreateTask { description, priority, agent_id }) => {
            let priority = match priority.to_lowercase().as_str() {
                "low" => TaskPriority::Low,
                "medium" => TaskPriority::Medium,
                "high" => TaskPriority::High,
                "critical" => TaskPriority::Critical,
                _ => TaskPriority::Medium,
            };
            
            if let Err(e) = handler.create_task(description, priority, agent_id).await {
                error!("Failed to create task: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::Tasks) => {
            // Implement task listing
            println!("Task listing not implemented yet");
        }
        Some(Commands::Workflows) => {
            match handler.list_workflows().await {
                Ok(workflows) => {
                    println!("\nWorkflows:");
                    for workflow in workflows {
                        println!("ID: {}", workflow.id);
                        println!("Name: {}", workflow.name);
                        println!("Status: {:?}", workflow.status);
                        println!("Steps: {}", workflow.steps.len());
                        println!("---");
                    }
                }
                Err(e) => {
                    error!("Failed to list workflows: {}", e);
                    process::exit(1);
                }
            }
        }
        Some(Commands::CreateWorkflow { name, steps }) => {
            let workflow_steps = steps.iter()
                .map(|step| WorkflowStep {
                    agent_id: "default".to_string(), // This should be configurable
                    action: AgentAction::ProcessText {
                        input: step.clone(),
                        max_tokens: 1000,
                    },
                    dependencies: Vec::new(),
                    timeout_seconds: Some(60),
                    retry_policy: Some(RetryPolicy::default()),
                })
                .collect();

            match handler.create_workflow(name, workflow_steps).await {
                Ok(workflow) => {
                    println!("Created workflow:");
                    println!("ID: {}", workflow.id);
                    println!("Name: {}", workflow.name);
                    println!("Steps: {}", workflow.steps.len());
                }
                Err(e) => {
                    error!("Failed to create workflow: {}", e);
                    process::exit(1);
                }
            }
        }
        Some(Commands::ExecuteWorkflow { id }) => {
            if let Err(e) = handler.execute_workflow(&id).await {
                error!("Failed to execute workflow: {}", e);
                process::exit(1);
            }
        }
        None => {
            println!("No command specified. Use --help for usage information.");
            process::exit(1);
        }
    }
} 