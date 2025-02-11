use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use serde_json;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum AgentAction {
    ProcessText { input: String, max_tokens: usize },
    GenerateCode { prompt: String, language: String },
    AnalyzeCode { code: String, aspects: Vec<String> },
    CustomTask { task_type: String, parameters: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub max_backoff_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum WorkflowStatus {
    Ready,
    Running,
    Completed,
    Failed,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorkflowStep {
    pub agent_id: String,
    pub action: AgentAction,
    pub dependencies: Vec<String>,
    pub retry_policy: Option<RetryPolicy>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentWorkflow {
    pub id: String,
    pub name: String,
    pub steps: Vec<WorkflowStep>,
    pub status: WorkflowStatus,
    #[schema(value_type = String, example = "2024-03-14T12:00:00Z")]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, nullable = true, example = "2024-03-14T13:00:00Z")]
    pub last_run: Option<DateTime<Utc>>,
} 