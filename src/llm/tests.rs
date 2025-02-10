use super::*;
use tokio::net::TcpListener;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;

async fn mock_llm_handler(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&hyper::Method::POST, "/v1/chat/completions") => {
            let response_json = json!({
                "id": "mock-response",
                "object": "chat.completion",
                "created": 1677858242,
                "model": "mock-model",
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "This is a mock response from the test server."
                    },
                    "finish_reason": "stop",
                    "index": 0
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 20,
                    "total_tokens": 30
                }
            });
            Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(Body::from(response_json.to_string()))
                .unwrap()
        },
        (&hyper::Method::POST, "/v1/functions") => {
            let response_json = json!({
                "result": {
                    "sum": 42
                }
            });
            Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(Body::from(response_json.to_string()))
                .unwrap()
        },
        _ => Response::builder()
            .status(404)
            .body(Body::from("Not Found"))
            .unwrap()
    };
    Ok(response)
}

async fn start_mock_server() -> SocketAddr {
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(addr).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(mock_llm_handler))
    });

    let server = Server::from_tcp(listener.into_std().unwrap()).unwrap();
    tokio::spawn(server.serve(make_svc));

    addr
}

#[tokio::test]
async fn test_llm_completion() {
    let addr = start_mock_server().await;
    let config = LLMConfig::with_lmstudio_server(format!("http://{}", addr));
    let client = LLMClient::new(config).unwrap();
    
    let response = client.complete("Test prompt").await;
    assert!(response.is_ok(), "Failed to get response: {:?}", response);
    let response = response.unwrap();
    assert!(response.contains("mock response"), "Response did not contain expected text: {}", response);
}

#[tokio::test]
async fn test_function_call() {
    let addr = start_mock_server().await;
    let config = LLMConfig::with_lmstudio_server(format!("http://{}", addr));
    let client = LLMClient::new(config).unwrap();
    
    #[derive(Serialize)]
    struct Args {
        x: i32,
        y: i32,
    }
    
    #[derive(Deserialize)]
    struct Result {
        sum: i32,
    }
    
    let response = client.call_function::<Args, Result>("add", &Args { x: 20, y: 22 }).await;
    assert!(response.is_ok(), "Failed to call function: {:?}", response);
    let result = response.unwrap();
    assert_eq!(result.sum, 42);
}

#[tokio::test]
async fn test_reasoning() {
    let addr = start_mock_server().await;
    let config = LLMConfig::with_lmstudio_server(format!("http://{}", addr));
    let client = LLMClient::new(config).unwrap();
    
    let response = client.reason("Test reasoning", None).await;
    assert!(response.is_ok(), "Failed to get reasoning: {:?}", response);
    let response = response.unwrap();
    assert!(response.contains("mock response"));
}

#[tokio::test]
async fn test_custom_server_connection() {
    let addr = start_mock_server().await;
    let config = LLMConfig::with_lmstudio_server(format!("http://{}", addr));
    let client = LLMClient::new(config).unwrap();
    
    let response = client.complete("Test custom server").await;
    assert!(response.is_ok(), "Failed to connect to custom server: {:?}", response);
    let response = response.unwrap();
    assert!(response.contains("mock response"));
}

// ... rest of the tests ... 