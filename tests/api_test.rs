use nexa_utils::api::ApiDoc;
use utoipa::OpenApi;
use serde_json::Value;

#[test]
fn test_api_documentation() {
    // Generate OpenAPI documentation
    let doc = ApiDoc::openapi();
    
    // Convert to JSON for validation
    let json = serde_json::to_value(&doc).expect("Failed to serialize OpenAPI doc");
    
    // Validate basic structure
    assert!(json.is_object());
    let obj = json.as_object().unwrap();
    
    // Check OpenAPI version
    assert_eq!(obj.get("openapi").unwrap().as_str().unwrap(), "3.0.3");
    
    // Check info section
    let info = obj.get("info").unwrap().as_object().unwrap();
    assert_eq!(info.get("title").unwrap().as_str().unwrap(), "Nexa Utils API");
    assert!(info.contains_key("version"));
    
    // Check paths
    let paths = obj.get("paths").unwrap().as_object().unwrap();
    assert!(paths.contains_key("/ws"));
    assert!(paths.contains_key("/agents/register"));
    assert!(paths.contains_key("/tasks/assign"));
    assert!(paths.contains_key("/agents/status"));
    assert!(paths.contains_key("/agents/query"));
    assert!(paths.contains_key("/metrics"));
    
    // Check components
    let components = obj.get("components").unwrap().as_object().unwrap();
    let schemas = components.get("schemas").unwrap().as_object().unwrap();
    
    // Validate required schemas
    assert!(schemas.contains_key("Agent"));
    assert!(schemas.contains_key("AgentStatus"));
    assert!(schemas.contains_key("Task"));
    assert!(schemas.contains_key("SystemMetrics"));
    assert!(schemas.contains_key("RegisterAgentRequest"));
    assert!(schemas.contains_key("TaskAssignmentRequest"));
    assert!(schemas.contains_key("StatusUpdateRequest"));
    assert!(schemas.contains_key("AgentQueryRequest"));
    
    // Check security schemes
    let security_schemes = components.get("securitySchemes").unwrap().as_object().unwrap();
    assert!(security_schemes.contains_key("bearer_auth"));
}

#[test]
fn test_schema_validation() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_value(&doc).expect("Failed to serialize OpenAPI doc");
    let schemas = json.get("components")
        .unwrap()
        .get("schemas")
        .unwrap()
        .as_object()
        .unwrap();
    
    // Test Agent schema
    let agent_schema = schemas.get("Agent").unwrap();
    validate_schema_properties(agent_schema, &["id", "capabilities", "status"]);
    
    // Test Task schema
    let task_schema = schemas.get("Task").unwrap();
    validate_schema_properties(task_schema, &["id", "task_type"]);
    
    // Test RegisterAgentRequest schema
    let register_schema = schemas.get("RegisterAgentRequest").unwrap();
    validate_schema_properties(register_schema, &["agent"]);
    
    // Test TaskAssignmentRequest schema
    let task_assignment_schema = schemas.get("TaskAssignmentRequest").unwrap();
    validate_schema_properties(task_assignment_schema, &["task", "agent_id"]);
}

fn validate_schema_properties(schema: &Value, required_props: &[&str]) {
    let properties = schema.get("properties").unwrap().as_object().unwrap();
    for prop in required_props {
        assert!(properties.contains_key(*prop), "Missing required property: {}", prop);
    }
    
    if let Some(required) = schema.get("required") {
        let required = required.as_array().unwrap();
        for prop in required_props {
            assert!(
                required.iter().any(|r| r.as_str().unwrap() == *prop),
                "Property {} should be required",
                prop
            );
        }
    }
}

#[test]
fn test_api_endpoints() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_value(&doc).expect("Failed to serialize OpenAPI doc");
    let paths = json.get("paths").unwrap().as_object().unwrap();
    
    // Test WebSocket endpoint
    let ws = paths.get("/ws").unwrap().as_object().unwrap();
    let get = ws.get("get").unwrap().as_object().unwrap();
    assert_eq!(get.get("tags").unwrap()[0], "System");
    
    // Test agent registration endpoint
    let register = paths.get("/agents/register").unwrap().as_object().unwrap();
    let post = register.get("post").unwrap().as_object().unwrap();
    assert_eq!(post.get("tags").unwrap()[0], "Agents");
    
    // Validate security requirements
    for (_, path) in paths {
        for (_, method) in path.as_object().unwrap() {
            let security = method.get("security").unwrap().as_array().unwrap();
            assert!(!security.is_empty(), "Endpoint missing security requirements");
        }
    }
} 