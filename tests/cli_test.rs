use log::info;
use once_cell::sync::OnceCell;
use std::sync::atomic::AtomicU16;
use nexa_core::cli::CliHandler;
use std::fs;
use nexa_core::types::agent::{AgentConfig, AgentStatus};
use nexa_core::types::workflow::{WorkflowStep, AgentAction};
use serde_json::json;

#[allow(dead_code)]
static PORT_COUNTER: AtomicU16 = AtomicU16::new(9000);

static TRACING: OnceCell<()> = OnceCell::new();

/// Initialize tracing for tests
fn init_tracing() {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .try_init()
            .unwrap_or_default();
    });
}

#[tokio::test]
async fn test_cli_handler() {
    init_tracing();

    let temp_dir = tempfile::tempdir().unwrap();
    let pid_file = temp_dir.path().join("nexa.pid");
    let state_file = temp_dir.path().join("nexa.state");
    let socket_path = temp_dir.path().join("nexa.sock");

    // Create necessary directories
    fs::create_dir_all(temp_dir.path().join("agents")).unwrap();
    fs::create_dir_all(temp_dir.path().join("workflows")).unwrap();

    // Change to temp directory for relative paths
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let cli = CliHandler::with_paths(pid_file, state_file, socket_path);
    
    // Test server start
    assert!(!cli.is_server_running());
    assert!(cli.start(None).await.is_ok());
    
    // Wait for server to start (up to 5 seconds)
    let mut attempts = 0;
    while !cli.is_server_running() && attempts < 50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        attempts += 1;
    }
    assert!(cli.is_server_running(), "Server failed to start after 5 seconds");
    
    // Test server status
    assert!(cli.status().await.is_ok());

    // Test agent creation
    let config = AgentConfig::default();
    let agent = cli.create_agent("test_agent".to_string(), config).await.unwrap();
    
    // Test agent listing
    let agents = cli.list_agents(None).await.unwrap();
    assert!(!agents.is_empty());
    assert_eq!(agents[0].name, "test_agent");

    // Test agent status filtering
    let active_agents = cli.list_agents(Some(AgentStatus::Active)).await.unwrap();
    assert!(active_agents.is_empty());

    // Test model listing
    let models = cli.list_models("lmstudio").await;
    assert!(models.is_ok() || models.unwrap_err().to_string().contains("not running"));

    // Test LLM server management
    assert!(cli.add_llm_server("test_provider", "http://localhost:1234").await.is_ok());
    assert!(cli.remove_llm_server("test_provider").await.is_ok());

    // Test task creation
    assert!(cli.create_task(
        "Test task".to_string(),
        nexa_core::llm::system_helper::TaskPriority::Medium,
        Some(agent.id.clone())
    ).await.is_ok());

    // Test workflow creation with a custom task that doesn't require LLM
    let workflow_step = WorkflowStep {
        agent_id: agent.id.clone(),
        action: AgentAction::CustomTask {
            task_type: "test".to_string(),
            parameters: json!({"test": true}),
        },
        dependencies: vec![],
        retry_policy: None,
        timeout_seconds: Some(1),
    };

    let workflow = cli.create_workflow(
        "test_workflow".to_string(),
        vec![workflow_step]
    ).await.unwrap();

    // Test workflow listing
    let workflows = cli.list_workflows().await.unwrap();
    assert!(!workflows.is_empty());
    assert_eq!(workflows[0].name, "test_workflow");

    // Test workflow execution
    assert!(cli.execute_workflow(&workflow.id).await.is_ok());

    // Set agent to Active before stopping
    assert!(cli.update_agent_status(&agent.id, AgentStatus::Active).await.is_ok());

    // Test agent stopping
    assert!(cli.stop_agent(&agent.id).await.is_ok());
    
    // Test server stop
    assert!(cli.stop().await.is_ok());
    
    // Wait for server to stop (up to 5 seconds)
    let mut attempts = 0;
    while cli.is_server_running() && attempts < 50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        attempts += 1;
    }
    assert!(!cli.is_server_running(), "Server failed to stop after 5 seconds");

    info!("CLI handler test completed successfully");
} 