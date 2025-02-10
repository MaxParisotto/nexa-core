use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::fmt;
use std::str::FromStr;

/// AgentStatus represents the current state of an agent (Idle, Active, Busy, Offline).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// Agent is idle and available for tasks
    Idle,
    /// Agent is currently working on a task
    Active,
    /// Agent is busy and cannot take new tasks
    Busy,
    /// Agent is offline and not available for tasks
    Offline,
}

impl Default for AgentStatus {
    fn default() -> Self {
        AgentStatus::Offline
    }
}

impl FromStr for AgentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "idle" => Ok(AgentStatus::Idle),
            "active" => Ok(AgentStatus::Active),
            "busy" => Ok(AgentStatus::Busy),
            "offline" => Ok(AgentStatus::Offline),
            _ => Err(format!("Invalid agent status: {}", s))
        }
    }
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStatus::Active => write!(f, "active"),
            AgentStatus::Busy => write!(f, "busy"),
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::Offline => write!(f, "offline"),
        }
    }
}

/// TaskStatus represents the current status of a task (Pending, InProgress, Completed, Failed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// ID of the parent agent, if any
    pub parent_id: Option<String>,
    /// IDs of child agents, if any
    pub children: Vec<String>,
    /// Timestamp of the last active time
    pub last_active: DateTime<Utc>,
    /// Configuration of the agent
    pub config: AgentConfig,
    /// Metrics of the agent
    pub metrics: AgentMetrics,
    /// Workflows this agent is part of
    pub workflows: Vec<String>,
    /// Supported actions this agent can perform
    pub supported_actions: Vec<String>,
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
            parent_id: None,
            children: Vec::new(),
            last_active: Utc::now(),
            config: AgentConfig::default(),
            metrics: AgentMetrics::default(),
            workflows: Vec::new(),
            supported_actions: Vec::new(),
        }
    }

    /// Check if the agent has a specific capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|cap| cap == capability)
    }
}

/// Task represents an assigned task with a description, priority, and associated agent id.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub llm_model: String,
    pub llm_provider: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            llm_model: "gpt-3.5-turbo".to_string(),
            llm_provider: "openai".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub uptime: u64,
    pub memory_usage: u64,
    pub cpu_usage: f64,
}

impl Default for AgentMetrics {
    fn default() -> Self {
        Self {
            tasks_completed: 0,
            tasks_failed: 0,
            uptime: 0,
            memory_usage: 0,
            cpu_usage: 0.0,
        }
    }
} 