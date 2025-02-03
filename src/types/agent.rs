use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use utoipa::ToSchema;

/// AgentStatus represents the current state of an agent (Idle or Active).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum AgentStatus {
    /// Agent is idle and available for tasks
    Idle,
    /// Agent is currently working on a task
    Active,
}

/// TaskStatus represents the current status of a task (Pending, InProgress, Completed, Failed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum TaskStatus {
    /// Task is waiting to be started
    Pending,
    /// Task is currently being executed
    InProgress,
    /// Task has been completed successfully
    Completed,
    /// Task has failed to complete
    Failed,
}

/// Agent encapsulates the properties of an agent.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Agent {
    /// Unique identifier for the agent
    pub id: String,
    /// Human-readable name of the agent
    pub name: String,
    /// List of capabilities this agent supports
    pub capabilities: Vec<String>,
    /// Current status of the agent
    pub status: AgentStatus,
    /// ID of the current task being executed, if any
    pub current_task: Option<String>,
    /// Timestamp of the last heartbeat received
    pub last_heartbeat: DateTime<Utc>,
}

impl Agent {
    /// Create a new agent with the given properties
    pub fn new(id: String, name: String, capabilities: Vec<String>) -> Self {
        Self {
            id,
            name,
            capabilities,
            status: AgentStatus::Idle,
            current_task: None,
            last_heartbeat: Utc::now(),
        }
    }

    /// Check if the agent has a specific capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|cap| cap == capability)
    }
}

/// Task represents an assigned task with a description, priority, and associated agent id.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Task {
    /// Unique identifier for the task
    pub id: String,
    /// Title of the task
    pub title: String,
    /// Detailed description of what needs to be done
    pub description: String,
    /// Current status of the task
    pub status: TaskStatus,
    /// Ordered list of steps to complete the task
    pub steps: Vec<String>,
    /// List of requirements needed to execute the task
    pub requirements: Vec<String>,
    /// ID of the agent assigned to this task, if any
    pub assigned_agent: Option<String>,
    /// When the task was created
    pub created_at: DateTime<Utc>,
    /// Optional deadline for task completion
    pub deadline: Option<DateTime<Utc>>,
    /// Task priority (higher number = higher priority)
    pub priority: i32,
    /// Estimated duration in seconds
    pub estimated_duration: i64,
}

impl Task {
    /// Create a new task with the given properties
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
            priority,
            estimated_duration,
        }
    }
} 