use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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