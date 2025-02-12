#![allow(dead_code, unused_imports, unused_variables)]

use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{Path, State},
    response::{IntoResponse, Response},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::cli::CliHandler;
use crate::types::agent::{AgentConfig, AgentStatus, Agent};
use crate::types::workflow::{WorkflowStep, AgentWorkflow};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use http_body_util::BodyExt;

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
pub struct StartServerRequest {
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServerResponse {
    pub success: bool,
    pub status: Option<String>,
    pub error: Option<String>,
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

#[derive(Debug, Clone)]
pub struct ApiError(pub String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0).into_response()
    }
}

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
            .route("/api/agents/{id}/stop", post(Self::stop_agent))
            .route("/api/agents/{id}/status/{status}", post(Self::update_agent_status))
            
            // LLM management
            .route("/api/llm/models/{provider}", get(Self::list_models))
            .route("/api/llm/servers", post(Self::add_llm_server))
            .route("/api/llm/servers/{provider}", get(Self::remove_llm_server))
            
            // Task management
            .route("/api/tasks", post(Self::create_task))
            .route("/api/tasks", get(Self::list_tasks))
            
            // Workflow management
            .route("/api/workflows", get(Self::list_workflows))
            .route("/api/workflows", post(Self::create_workflow))
            .route("/api/workflows/{id}/execute", post(Self::execute_workflow))
            .with_state(cli);

        // Create Swagger UI router
        let swagger_ui = SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", ApiDoc::openapi());

        // Merge the routers
        Router::new()
            .merge(api_router)
            .merge(swagger_ui)
    }

    // Server management handlers
    async fn start_server(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(request): Json<StartServerRequest>,
    ) -> impl IntoResponse {
        let cli = cli.write().await;
        match cli.start(request.port.map(|p| p.to_string()).as_deref()).await {
            Ok(_) => Json(ServerResponse {
                success: true,
                status: Some("running".to_string()),
                error: None,
            }).into_response(),
            Err(e) => (
                StatusCode::OK,
                Json(ServerResponse {
                    success: false,
                    status: None,
                    error: Some(e.to_string()),
                })
            ).into_response(),
        }
    }

    async fn stop_server(
        State(cli): State<Arc<RwLock<CliHandler>>>,
    ) -> impl IntoResponse {
        let cli = cli.write().await;
        match cli.stop().await {
            Ok(_) => Json(ServerResponse {
                success: true,
                status: Some("stopped".to_string()),
                error: None,
            }).into_response(),
            Err(e) => (
                StatusCode::OK,
                Json(ServerResponse {
                    success: false,
                    status: None,
                    error: Some(e.to_string()),
                })
            ).into_response(),
        }
    }

    async fn get_server_status(
        State(cli): State<Arc<RwLock<CliHandler>>>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        let status = if cli.is_server_running() {
            "running".to_string()
        } else {
            "stopped".to_string()
        };
        Json(ServerStatusResponse {
            status,
            cpu_usage: 0.0,
            memory_usage: 0.0,
        }).into_response()
    }

    // Agent management handlers
    async fn list_agents(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(status): Json<Option<AgentStatus>>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        match cli.list_agents(status).await {
            Ok(agents) => Json(ApiResponse::<Vec<Agent>> {
                success: true,
                data: Some(agents),
                error: None,
            }),
            Err(e) => Json(ApiResponse::<Vec<Agent>> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn create_agent(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(req): Json<AgentRequest>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        let mut config = AgentConfig::default();
        if let Some(model) = req.model {
            config.llm_model = model;
        }
        if let Some(provider) = req.provider {
            config.llm_provider = provider;
        }

        match cli.create_agent(req.name, config).await {
            Ok(agent) => Json(ApiResponse::<Agent> {
                success: true,
                data: Some(agent),
                error: None,
            }),
            Err(e) => Json(ApiResponse::<Agent> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn stop_agent(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path(id): Path<String>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        match cli.stop_agent(&id).await {
            Ok(_) => Json(ApiResponse::<()> {
                success: true,
                data: None,
                error: None,
            }),
            Err(e) => Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn update_agent_status(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path((id, status)): Path<(String, AgentStatus)>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        match cli.update_agent_status(&id, status).await {
            Ok(_) => Json(ApiResponse::<()> {
                success: true,
                data: None,
                error: None,
            }),
            Err(e) => Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    // LLM management handlers
    async fn list_models(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path(provider): Path<String>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        match cli.list_models(&provider).await {
            Ok(models) => Json(ApiResponse::<Vec<crate::cli::LLMModel>> {
                success: true,
                data: Some(models),
                error: None,
            }),
            Err(e) => Json(ApiResponse::<Vec<crate::cli::LLMModel>> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn add_llm_server(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(req): Json<(String, String)>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        match cli.add_llm_server(&req.0, &req.1).await {
            Ok(_) => Json(ApiResponse::<()> {
                success: true,
                data: None,
                error: None,
            }),
            Err(e) => Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn remove_llm_server(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path(provider): Path<String>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        match cli.remove_llm_server(&provider).await {
            Ok(_) => Json(ApiResponse::<()> {
                success: true,
                data: None,
                error: None,
            }),
            Err(e) => Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    // Task management handlers
    async fn create_task(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(req): Json<TaskRequest>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        let priority = match req.priority.to_lowercase().as_str() {
            "low" => crate::llm::system_helper::TaskPriority::Low,
            "medium" => crate::llm::system_helper::TaskPriority::Medium,
            "high" => crate::llm::system_helper::TaskPriority::High,
            "critical" => crate::llm::system_helper::TaskPriority::Critical,
            _ => crate::llm::system_helper::TaskPriority::Medium,
        };

        match cli.create_task(req.description, priority, req.agent_id).await {
            Ok(_) => Json(ApiResponse::<()> {
                success: true,
                data: None,
                error: None,
            }),
            Err(e) => Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn list_tasks(
        State(cli): State<Arc<RwLock<CliHandler>>>,
    ) -> impl IntoResponse {
        Json(ApiResponse::<()> {
            success: true,
            data: None,
            error: None,
        })
    }

    // Workflow management handlers
    async fn list_workflows(
        State(cli): State<Arc<RwLock<CliHandler>>>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        match cli.list_workflows().await {
            Ok(workflows) => Json(ApiResponse::<Vec<AgentWorkflow>> {
                success: true,
                data: Some(workflows),
                error: None,
            }),
            Err(e) => Json(ApiResponse::<Vec<AgentWorkflow>> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn create_workflow(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Json(req): Json<WorkflowRequest>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        match cli.create_workflow(req.name, req.steps).await {
            Ok(workflow) => Json(ApiResponse::<AgentWorkflow> {
                success: true,
                data: Some(workflow),
                error: None,
            }),
            Err(e) => Json(ApiResponse::<AgentWorkflow> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn execute_workflow(
        State(cli): State<Arc<RwLock<CliHandler>>>,
        Path(id): Path<String>,
    ) -> impl IntoResponse {
        let cli = cli.read().await;
        match cli.execute_workflow(&id).await {
            Ok(_) => Json(ApiResponse::<()> {
                success: true,
                data: None,
                error: None,
            }),
            Err(e) => Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        http::Request,
        body::Body,
    };
    use tower::ServiceExt;
    use serde_json::json;
    use http_body_util::BodyExt;
    use std::env;

    async fn setup_test_api() -> Router {
        let temp_dir = env::temp_dir();
        let cli = CliHandler::with_paths(
            temp_dir.join("nexa.pid"),
            temp_dir.join("nexa.state"),
            temp_dir.join("nexa.sock"),
        );
        let api = ApiServer::new(cli);
        api.router()
    }

    #[tokio::test]
    async fn test_server_status() {
        let app = setup_test_api().await;

        // Test server status endpoint
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/server/status")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        
        // Convert response body to bytes
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let response: ServerStatusResponse = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(response.status, "stopped");
        assert_eq!(response.cpu_usage, 0.0);
        assert_eq!(response.memory_usage, 0.0);
    }

    #[tokio::test]
    async fn test_server_start_stop() {
        let app = setup_test_api().await;

        // Test server start
        let start_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/server/start")
                    .method("POST")
                    .header("Content-Type", "application/json")
                    .body(Body::from(json!({
                        "port": 3000
                    }).to_string()))
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(start_response.status(), StatusCode::OK);
        
        // Test server stop
        let stop_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/server/stop")
                    .method("POST")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(stop_response.status(), StatusCode::OK);
    }
} 
