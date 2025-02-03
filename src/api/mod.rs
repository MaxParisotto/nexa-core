use utoipa::OpenApi;
use crate::types::{Agent, AgentStatus, Task};
use crate::monitoring::SystemMetrics;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Agent registration request
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RegisterAgentRequest {
    /// Agent information
    pub agent: Agent,
}

/// Task assignment request
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskAssignmentRequest {
    /// Task information
    pub task: Task,
    /// Target agent ID
    pub agent_id: String,
    /// Optional deadline
    pub deadline: Option<DateTime<Utc>>,
}

/// Status update request
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct StatusUpdateRequest {
    /// Agent ID
    pub agent_id: String,
    /// Current status
    pub status: AgentStatus,
    /// Optional metrics
    pub metrics: Option<HashMap<String, String>>,
}

/// Agent query request
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct AgentQueryRequest {
    /// Required capability
    pub capability: String,
    /// Additional requirements
    pub requirements: Option<HashMap<String, String>>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        ws_connect,
        register_agent,
        assign_task,
        update_status,
        query_agents,
        get_metrics
    ),
    components(
        schemas(
            Agent,
            AgentStatus,
            Task,
            SystemMetrics,
            RegisterAgentRequest,
            TaskAssignmentRequest,
            StatusUpdateRequest,
            AgentQueryRequest
        )
    ),
    tags(
        (name = "Agents", description = "Agent management operations"),
        (name = "Tasks", description = "Task management operations"),
        (name = "System", description = "System monitoring and control"),
        (name = "Metrics", description = "Resource and performance metrics")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer
                    )
                ),
            );
        }
        
        openapi.info.title = "Nexa Utils API".to_string();
        openapi.info.version = "1.0.0".to_string();
        openapi.info.description = Some("Multi-agent Control Protocol (MCP) Implementation".to_string());
    }
}

/// WebSocket connection endpoint
#[utoipa::path(
    get,
    path = "/ws",
    tag = "System",
    responses(
        (status = 101, description = "WebSocket handshake successful"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn ws_connect() {}

/// Register a new agent
#[utoipa::path(
    post,
    path = "/agents/register",
    tag = "Agents",
    request_body = RegisterAgentRequest,
    responses(
        (status = 200, description = "Agent registered successfully"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn register_agent() {}

/// Assign a task to an agent
#[utoipa::path(
    post,
    path = "/tasks/assign",
    tag = "Tasks",
    request_body = TaskAssignmentRequest,
    responses(
        (status = 200, description = "Task assigned successfully"),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Agent not found"),
        (status = 500, description = "Server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn assign_task() {}

/// Update agent status
#[utoipa::path(
    post,
    path = "/agents/status",
    tag = "Agents",
    request_body = StatusUpdateRequest,
    responses(
        (status = 200, description = "Status updated successfully"),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Agent not found"),
        (status = 500, description = "Server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_status() {}

/// Query agents by capability
#[utoipa::path(
    post,
    path = "/agents/query",
    tag = "Agents",
    request_body = AgentQueryRequest,
    responses(
        (status = 200, description = "Query successful", body = Vec<Agent>),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn query_agents() {}

/// Get system metrics
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "Metrics",
    responses(
        (status = 200, description = "Metrics retrieved successfully", body = SystemMetrics),
        (status = 500, description = "Server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_metrics() {} 
