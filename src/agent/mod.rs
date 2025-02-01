use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use utoipa::ToSchema;

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
    pub estimated_duration: i64,
    pub priority: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum AgentStatus {
    Idle,
    Busy,
    Offline,
    Error,
}

impl Task {
    pub fn new(
        title: String,
        description: String,
        steps: Vec<String>,
        requirements: Vec<String>,
        deadline: Option<DateTime<Utc>>,
        estimated_duration: i64,
        priority: i32,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            description,
            status: TaskStatus::Pending,
            steps,
            requirements,
            assigned_agent: None,
            created_at: Utc::now(),
            deadline,
            estimated_duration,
            priority,
        }
    }
}

impl Agent {
    pub fn new(name: String, capabilities: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            capabilities,
            status: AgentStatus::Idle,
            current_task: None,
            last_heartbeat: Utc::now(),
        }
    }

    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Utc::now();
    }

    pub fn assign_task(&mut self, task_id: String) {
        self.current_task = Some(task_id);
        self.status = AgentStatus::Busy;
    }

    pub fn complete_task(&mut self) {
        self.current_task = None;
        self.status = AgentStatus::Idle;
    }

    pub fn set_status(&mut self, status: AgentStatus) {
        self.status = status;
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|cap| cap == capability)
    }
} 