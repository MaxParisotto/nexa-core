use clap::Parser;
use log::{info, error};
use std::process;
use tokio::time::Duration;
use std::io::{stdout, Write};
use crossterm::{
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
    cursor::{Hide, Show},
    style::{Color, Print, SetForegroundColor},
};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use daemonize::Daemonize;
use std::fs::File;
use std::path::PathBuf;
use std::os::unix::fs::PermissionsExt;
use crate::server::ServerState;

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

use crate::cli::{Cli, Commands, CliHandler};
use crate::types::agent::AgentConfig;
use crate::llm::system_helper::TaskPriority;
use crate::types::workflow::{WorkflowStep, AgentAction, RetryPolicy};

async fn display_stats_dashboard(_: &CliHandler) -> Result<(), Box<dyn std::error::Error>> {
    execute!(stdout(), EnterAlternateScreen, Hide)?;

    loop {
        execute!(
            stdout(),
            Clear(ClearType::All),
            SetForegroundColor(Color::Cyan),
            Print("\n  NEXA Core System Monitor\n"),
            SetForegroundColor(Color::White),
            Print("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n"),
            SetForegroundColor(Color::Green),
            Print(format!("  System Time:   {}\n", chrono::Local::now().format("%H:%M:%S"))),
            SetForegroundColor(Color::White),
            Print("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"),
            Print("\n  Press Ctrl+C to exit\n"),
        )?;

        stdout().flush()?;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init();

    // Parse command line arguments
    let cli = Cli::parse();
    let handler = CliHandler::new();

    // Setup Ctrl+C handler
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    ctrlc::set_handler(move || {
        let _ = tx.blocking_send(());
    }).expect("Error setting Ctrl-C handler");

    match cli.command {
        Some(cmd) => {
            process_command(&cmd, &handler).await;
        }
        None => {
            // Show real-time dashboard
            let dashboard = display_stats_dashboard(&handler);
            tokio::select! {
                _ = dashboard => {},
                _ = rx.recv() => {
                    // Cleanup and restore terminal
                    execute!(stdout(), Show, LeaveAlternateScreen).unwrap_or(());
                }
            }
        }
    }
}

async fn process_command(command: &Commands, handler: &CliHandler) {
    match command {
        Commands::Start { port } => {
            // Create runtime directory if needed
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            let runtime_dir = PathBuf::from(&home).join(".nexa");
            std::fs::create_dir_all(&runtime_dir).expect("Failed to create runtime directory");
            
            let log_file = File::create(runtime_dir.join("nexa.log"))
                .expect("Failed to create log file");
            let err_file = File::create(runtime_dir.join("nexa.err"))
                .expect("Failed to create error file");
            
            // Set file permissions
            let mode = 0o644;
            let _ = log_file.metadata().map(|m| {
                let mut perms = m.permissions();
                perms.set_mode(mode);
                log_file.set_permissions(perms)
            });
            let _ = err_file.metadata().map(|m| {
                let mut perms = m.permissions();
                perms.set_mode(mode);
                err_file.set_permissions(perms)
            });
            
            // Create daemon
            let daemonize: Daemonize<()> = Daemonize::new()
                .pid_file(runtime_dir.join("nexa.pid"))
                .working_directory(&runtime_dir)
                .stdout(log_file)
                .stderr(err_file)
                .privileged_action(|| {
                    unsafe { libc::setsid() };
                    println!("Starting Nexa server daemon...");
                });
            
            match daemonize.start() {
                Ok(_) => {
                    // We are now in the daemon process
                    
                    // Write initial state
                    std::fs::write(runtime_dir.join("nexa.state"), "Starting")
                        .expect("Failed to write state file");
                    
                    // Create a new tokio runtime for the daemon
                    let runtime = tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to create tokio runtime");
                    
                    // Run server in the new runtime
                    runtime.block_on(async {
                        // Start server
                        if let Err(e) = handler.start(port.map(|p| p.to_string()).as_deref()).await {
                            error!("Failed to start server: {}", e);
                            let _ = std::fs::remove_file(runtime_dir.join("nexa.pid"));
                            let _ = std::fs::remove_file(runtime_dir.join("nexa.state"));
                            std::process::exit(1);
                        }
                        
                        // Update state to running
                        std::fs::write(runtime_dir.join("nexa.state"), "Running")
                            .expect("Failed to write state file");
                        
                        // Keep server running
                        let running = Arc::new(AtomicBool::new(true));
                        let r = running.clone();
                        
                        ctrlc::set_handler(move || {
                            r.store(false, Ordering::SeqCst);
                        }).expect("Error setting Ctrl-C handler");
                        
                        // Spawn state monitoring task
                        let handler_clone = handler.clone();
                        let running_clone = running.clone();
                        let state_task = tokio::spawn(async move {
                            while running_clone.load(Ordering::SeqCst) {
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                if let Err(e) = handler_clone.save_server_state(ServerState::Running).await {
                                    error!("Failed to update server state: {}", e);
                                }
                            }
                        });
                        
                        // Wait for shutdown signal
                        while running.load(Ordering::SeqCst) {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        
                        // Wait for state task
                        let _ = state_task.await;
                        
                        // Clean shutdown
                        if let Err(e) = handler.stop().await {
                            error!("Error stopping server: {}", e);
                        }
                        
                        // Clean up files
                        let _ = std::fs::remove_file(runtime_dir.join("nexa.pid"));
                        let _ = std::fs::remove_file(runtime_dir.join("nexa.state"));
                    });
                }
                Err(e) => {
                    error!("Failed to start daemon: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Stop => {
            info!("Stopping server...");
            if let Err(e) = handler.stop().await {
                error!("Failed to stop server: {}", e);
                process::exit(1);
            }
        }
        Commands::Status => {
            info!("Checking server status...");
            if let Err(e) = handler.status().await {
                error!("Failed to get server status: {}", e);
                process::exit(1);
            }
        }
        Commands::Agents { status } => {
            let status_enum = status.as_ref().map(|s| s.parse().unwrap_or_default());
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
        Commands::CreateAgent { name, model, provider } => {
            let mut config = AgentConfig::default();
            if let Some(m) = model {
                config.llm_model = m.to_string();
            }
            if let Some(p) = provider {
                config.llm_provider = p.to_string();
            }
            match handler.create_agent(name.to_string(), config).await {
                Ok(agent) => println!("Created agent: {}", agent.id),
                Err(e) => {
                    error!("Failed to create agent: {}", e);
                    process::exit(1);
                }
            }
        }
        Commands::StopAgent { id } => {
            if let Err(e) = handler.stop_agent(&id).await {
                error!("Failed to stop agent: {}", e);
                process::exit(1);
            }
        }
        Commands::Models { provider } => {
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
        Commands::AddServer { provider, url } => {
            if let Err(e) = handler.add_llm_server(&provider, &url).await {
                error!("Failed to add server: {}", e);
                process::exit(1);
            }
        }
        Commands::RemoveServer { provider } => {
            if let Err(e) = handler.remove_llm_server(&provider).await {
                error!("Failed to remove server: {}", e);
                process::exit(1);
            }
        }
        Commands::CreateTask { description, priority, agent_id } => {
            let priority = match priority.to_lowercase().as_str() {
                "low" => TaskPriority::Low,
                "medium" => TaskPriority::Medium,
                "high" => TaskPriority::High,
                "critical" => TaskPriority::Critical,
                _ => TaskPriority::Medium,
            };
            if let Err(e) = handler.create_task(description.clone(), priority, agent_id.clone()).await {
                error!("Failed to create task: {}", e);
                process::exit(1);
            }
        }
        Commands::Tasks => {
            println!("Task listing not implemented yet");
        }
        Commands::Workflows => {
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
        Commands::CreateWorkflow { name, steps } => {
            let workflow_steps = steps.iter()
                .map(|step| WorkflowStep {
                    agent_id: "default".to_string(),
                    action: AgentAction::ProcessText {
                        input: step.clone(),
                        max_tokens: 1000,
                    },
                    dependencies: Vec::new(),
                    timeout_seconds: Some(60),
                    retry_policy: Some(RetryPolicy::default()),
                })
                .collect();
            match handler.create_workflow(name.clone(), workflow_steps).await {
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
        Commands::ExecuteWorkflow { id } => {
            if let Err(e) = handler.execute_workflow(&id).await {
                error!("Failed to execute workflow: {}", e);
                process::exit(1);
            }
        }
    }
} 