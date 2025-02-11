#![allow(dead_code, unused_imports, unused_variables)]

use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{Path, State},
    http::StatusCode,
    body::Body,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::cli::CliHandler;
use crate::types::agent::{AgentConfig, AgentStatus};
use crate::types::workflow::{WorkflowStep, AgentWorkflow};
use crate::error::NexaError;
use sysinfo::System;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentRequest {
    #[schema(example = "test_agent")]
    pub name: String,
    #[schema(example = "gpt-4")]
    pub model: Option<String>,
    #[schema(example = "openai")]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskRequest {
    #[schema(example = "Analyze code for security issues")]
    pub description: String,
    #[schema(example = "high")]
    pub priority: String,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorkflowRequest {
    #[schema(example = "code_analysis")]
    pub name: String,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServerStatusResponse {
    #[schema(example = "Running")]
    pub status: String,
    #[schema(example = 45.5)]
    pub cpu_usage: f32,
    #[schema(example = 60.2)]
    pub memory_usage: f32,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Nexa Core API",
        version = "1.0.0",
        description = "REST API for managing Nexa Core server, agents, and workflows",
        contact(
            name = "Nexa Team",
            url = "https://github.com/nexa-core"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    components(
        schemas(
            ApiResponse<ServerStatusResponse>,
            ApiResponse<Vec<crate::types::agent::Agent>>,
            ApiResponse<crate::types::agent::Agent>,
            ApiResponse<Vec<crate::cli::LLMModel>>,
            ApiResponse<AgentWorkflow>,
            AgentRequest,
            TaskRequest,
            WorkflowRequest,
            ServerStatusResponse
        )
    ),
    tags(
        (name = "server", description = "Server management endpoints"),
        (name = "agents", description = "Agent management endpoints"),
        (name = "llm", description = "LLM management endpoints"),
        (name = "tasks", description = "Task management endpoints"),
        (name = "workflows", description = "Workflow management endpoints")
    ),
    servers(
        (url = "http://localhost:3000", description = "Local development server")
    ),
    security(
        ()
    ),
    external_docs(
        url = "https://github.com/nexa-core/docs",
        description = "Find more information here"
    )
)]
pub struct ApiDoc;

pub struct ApiServer {
    cli: Arc<RwLock<CliHandler>>,
}

impl ApiServer {
    pub fn new(cli: CliHandler) -> Self {
        Self {
            cli: Arc::new(RwLock::new(cli)),
        }
    }

    pub fn router(&self) -> Router {
        let cli = self.cli.clone();

        let api_router = Router::new()
            // Server management
            .route("/api/server/start", post(Self::start_server))
            .route("/api/server/stop", post(Self::stop_server))
            .route("/api/server/status", get(Self::get_server_status))
            
            // Agent management
            .route("/api/agents", get(Self::list_agents))
            .route("/api/agents", post(Self::create_agent))
            .route("/api/agents/:id/stop", post(Self::stop_agent))
            .route("/api/agents/:id/status/:status", post(Self::update_agent_status))
            
            // LLM management
            .route("/api/llm/models/:provider", get(Self::list_models))
            .route("/api/llm/servers", post(Self::add_llm_server))
            .route("/api/llm/servers/:provider", get(Self::remove_llm_server))
            
            // Task management
            .route("/api/tasks", post(Self::create_task))
            .route("/api/tasks", get(Self::list_tasks))
            
            // Workflow management
            .route("/api/workflows", get(Self::list_workflows))
            .route("/api/workflows", post(Self::create_workflow))
            .route("/api/workflows/:id/execute", post(Self::execute_workflow))
            .with_state(cli);

        // Create Swagger UI router
        let swagger_ui = SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", ApiDoc::openapi());

        // Merge the routers
        Router::new()
            .merge(api_router)
            .merge(swagger_ui)
    }

    async fn start_server(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(port): Json<Option<u16>>,
    ) -> Result<Json<ApiResponse<()>>, StatusCode> {
        let cli = cli.read().await;
        match cli.start(port.map(|p| p.to_string()).as_deref()).await {
            Ok(_) => Ok(Json(ApiResponse {
                success: true,
                data: None,
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn stop_server(
        State(cli): State<Arc<RwLock<CliHandler>>>,
    ) -> Result<Json<ApiResponse<()>>, StatusCode> {
        let cli = cli.read().await;
        match cli.stop().await {
            Ok(_) => Ok(Json(ApiResponse {
                success: true,
                data: None,
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn get_server_status(
        State(cli): State<Arc<RwLock<CliHandler>>>,
    ) -> Result<Json<ApiResponse<ServerStatusResponse>>, StatusCode> {
        let cli = cli.read().await;
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();

        let status = if cli.is_server_running() {
            "Running"
        } else {
            "Stopped"
        };

        Ok(Json(ApiResponse {
            success: true,
            data: Some(ServerStatusResponse {
                status: status.to_string(),
                cpu_usage: sys.global_cpu_usage(),
                memory_usage: sys.used_memory() as f32 / sys.total_memory() as f32 * 100.0,
            }),
            error: None,
        }))
    }

    async fn list_agents(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(status): Json<Option<AgentStatus>>,
    ) -> Result<Json<ApiResponse<Vec<crate::types::agent::Agent>>>, StatusCode> {
        let cli = cli.read().await;
        match cli.list_agents(status).await {
            Ok(agents) => Ok(Json(ApiResponse {
                success: true,
                data: Some(agents),
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn create_agent(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(req): Json<AgentRequest>,
    ) -> Result<Json<ApiResponse<crate::types::agent::Agent>>, StatusCode> {
        let cli = cli.read().await;
        let mut config = AgentConfig::default();
        if let Some(model) = req.model {
            config.llm_model = model;
        }
        if let Some(provider) = req.provider {
            config.llm_provider = provider;
        }

        match cli.create_agent(req.name, config).await {
            Ok(agent) => Ok(Json(ApiResponse {
                success: true,
                data: Some(agent),
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn stop_agent(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path(id): Path<String>,
    ) -> Result<Json<ApiResponse<()>>, StatusCode> {
        let cli = cli.read().await;
        match cli.stop_agent(&id).await {
            Ok(_) => Ok(Json(ApiResponse {
                success: true,
                data: None,
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn update_agent_status(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path((id, status)): Path<(String, AgentStatus)>,
    ) -> Result<Json<ApiResponse<()>>, StatusCode> {
        let cli = cli.read().await;
        match cli.update_agent_status(&id, status).await {
            Ok(_) => Ok(Json(ApiResponse {
                success: true,
                data: None,
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn list_models(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path(provider): Path<String>,
    ) -> Result<Json<ApiResponse<Vec<crate::cli::LLMModel>>>, StatusCode> {
        let cli = cli.read().await;
        match cli.list_models(&provider).await {
            Ok(models) => Ok(Json(ApiResponse {
                success: true,
                data: Some(models),
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn add_llm_server(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(req): Json<(String, String)>,
    ) -> Result<Json<ApiResponse<()>>, StatusCode> {
        let cli = cli.read().await;
        match cli.add_llm_server(&req.0, &req.1).await {
            Ok(_) => Ok(Json(ApiResponse {
                success: true,
                data: None,
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn remove_llm_server(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path(provider): Path<String>,
    ) -> Result<Json<ApiResponse<()>>, StatusCode> {
        let cli = cli.read().await;
        match cli.remove_llm_server(&provider).await {
            Ok(_) => Ok(Json(ApiResponse {
                success: true,
                data: None,
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn create_task(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(req): Json<TaskRequest>,
    ) -> Result<Json<ApiResponse<()>>, StatusCode> {
        let cli = cli.read().await;
        let priority = match req.priority.to_lowercase().as_str() {
            "low" => crate::llm::system_helper::TaskPriority::Low,
            "medium" => crate::llm::system_helper::TaskPriority::Medium,
            "high" => crate::llm::system_helper::TaskPriority::High,
            "critical" => crate::llm::system_helper::TaskPriority::Critical,
            _ => crate::llm::system_helper::TaskPriority::Medium,
        };

        match cli.create_task(req.description, priority, req.agent_id).await {
            Ok(_) => Ok(Json(ApiResponse {
                success: true,
                data: None,
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn list_tasks(
        State(cli): State<Arc<RwLock<CliHandler>>>,
    ) -> Result<Json<ApiResponse<()>>, StatusCode> {
        // TODO: Implement task listing
        Ok(Json(ApiResponse {
            success: true,
            data: None,
            error: None,
        }))
    }

    async fn list_workflows(
        State(cli): State<Arc<RwLock<CliHandler>>>,
    ) -> Result<Json<ApiResponse<Vec<AgentWorkflow>>>, StatusCode> {
        let cli = cli.read().await;
        match cli.list_workflows().await {
            Ok(workflows) => Ok(Json(ApiResponse {
                success: true,
                data: Some(workflows),
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn create_workflow(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(req): Json<WorkflowRequest>,
    ) -> Result<Json<ApiResponse<AgentWorkflow>>, StatusCode> {
        let cli = cli.read().await;
        match cli.create_workflow(req.name, req.steps).await {
            Ok(workflow) => Ok(Json(ApiResponse {
                success: true,
                data: Some(workflow),
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }

    async fn execute_workflow(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path(id): Path<String>,
    ) -> Result<Json<ApiResponse<()>>, StatusCode> {
        let cli = cli.read().await;
        match cli.execute_workflow(&id).await {
            Ok(_) => Ok(Json(ApiResponse {
                success: true,
                data: None,
                error: None,
            })),
            Err(e) => Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_api_responses() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cli = CliHandler::with_paths(
            temp_dir.path().join("nexa.pid"),
            temp_dir.path().join("nexa.state"),
            temp_dir.path().join("nexa.sock"),
        );
        let api = ApiServer::new(cli);
        let app = api.router();

        // Test server status endpoint
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/server/status")
                    .method("GET")
                    .body(Body::empty())
                    .expect("Failed to build request")
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
} 
