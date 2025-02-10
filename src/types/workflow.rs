use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use serde_json;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Ready,
    Running,
    Completed,
    Failed,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub agent_id: String,
    pub action: AgentAction,
    pub dependencies: Vec<String>,
    pub retry_policy: Option<RetryPolicy>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWorkflow {
    pub id: String,
    pub name: String,
    pub steps: Vec<WorkflowStep>,
    pub status: WorkflowStatus,
    pub created_at: DateTime<Utc>,
    pub last_run: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentAction {
    ProcessText { input: String, max_tokens: usize },
    GenerateCode { prompt: String, language: String },
    AnalyzeCode { code: String, aspects: Vec<String> },
    CustomTask { task_type: String, parameters: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub max_backoff_ms: u64,
} 