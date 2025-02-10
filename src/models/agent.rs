use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::fmt;
use crate::types::agent::{AgentStatus as TypesAgentStatus, AgentConfig, AgentMetrics, Agent as TypesAgent};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModelAgentStatus {
    Active,
    Idle,
}

impl fmt::Display for ModelAgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelAgentStatus::Active => write!(f, "active"),
            ModelAgentStatus::Idle => write!(f, "idle"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub status: ModelAgentStatus,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub current_task: Option<String>,
    pub last_heartbeat: DateTime<Utc>,
    pub config: AgentConfig,
    pub metrics: AgentMetrics,
    pub workflows: Vec<String>,
    pub supported_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub steps: Vec<String>,
    pub requirements: Vec<String>,
    pub assigned_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub deadline: Option<DateTime<Utc>>,
    pub priority: TaskPriority,
    pub estimated_duration: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl Agent {
    pub fn from_cli_agent(cli_agent: TypesAgent) -> Self {
        Self {
            id: cli_agent.id,
            name: cli_agent.name,
            capabilities: cli_agent.capabilities,
            status: match cli_agent.status {
                TypesAgentStatus::Active | TypesAgentStatus::Busy => ModelAgentStatus::Active,
                _ => ModelAgentStatus::Idle,
            },
            parent_id: cli_agent.parent_id,
            children: cli_agent.children,
            current_task: cli_agent.current_task,
            last_heartbeat: cli_agent.last_heartbeat,
            config: cli_agent.config,
            metrics: cli_agent.metrics,
            workflows: cli_agent.workflows,
            supported_actions: cli_agent.supported_actions,
        }
    }
} 