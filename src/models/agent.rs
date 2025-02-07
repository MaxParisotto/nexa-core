use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Ord, PartialOrd)]
pub enum AgentStatus {
    Idle,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub status: AgentStatus,
    pub current_task: Option<String>,
    pub last_heartbeat: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
    pub priority: i32,
    pub estimated_duration: i64,
}

impl Agent {
    pub fn from_cli_agent(cli_agent: crate::cli::Agent) -> Self {
        Self {
            id: cli_agent.id,
            name: cli_agent.name,
            capabilities: cli_agent.capabilities,
            status: match cli_agent.status {
                crate::cli::AgentStatus::Active | crate::cli::AgentStatus::Busy => AgentStatus::Active,
                _ => AgentStatus::Idle,
            },
            current_task: None, // Could be derived from workflows if needed
            last_heartbeat: cli_agent.last_active,
        }
    }
} 