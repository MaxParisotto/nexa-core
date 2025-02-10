use nexa_core::api::{ApiResponse, AgentRequest, TaskRequest};

#[tokio::test]
async fn test_api_responses() {
    // Test agent request
    let _agent_req = AgentRequest {
        name: "test_agent".to_string(),
        model: Some("gpt-3.5-turbo".to_string()),
        provider: Some("openai".to_string()),
    };

    // Test task request
    let _task_req = TaskRequest {
        description: "test task".to_string(),
        priority: "medium".to_string(),
        agent_id: None,
    };

    // Test successful response
    let success_response: ApiResponse<String> = ApiResponse {
        success: true,
        data: Some("test data".to_string()),
        error: None,
    };
    assert!(success_response.success);
    assert!(success_response.data.is_some());
    assert!(success_response.error.is_none());

    // Test error response
    let error_response: ApiResponse<()> = ApiResponse {
        success: false,
        data: None,
        error: Some("test error".to_string()),
    };
    assert!(!error_response.success);
    assert!(error_response.data.is_none());
    assert!(error_response.error.is_some());
} 