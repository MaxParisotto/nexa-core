use std::net::SocketAddr;
use axum::{
    routing::post,
    Router,
    Json,
    http::StatusCode,
};
use serde_json::json;
use tokio::net::TcpListener;
use log::error;

pub async fn start_mock_server() -> SocketAddr {
    let app = Router::new()
        .route("/v1/chat/completions", post(mock_llm_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener");
    let addr = listener.local_addr().expect("Failed to get local address");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .unwrap_or_else(|e| error!("Server error: {}", e));
    });

    addr
}

async fn mock_llm_handler() -> (StatusCode, Json<serde_json::Value>) {
    let response_json = json!({
        "id": "mock-response",
        "object": "chat.completion",
        "created": 1677858242,
        "model": "mock-model",
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "fn add(a: i32, b: i32) -> i32 { a + b }"
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

    (StatusCode::OK, Json(response_json))
} 