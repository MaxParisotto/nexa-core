use tokio::net::TcpListener;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;

pub async fn start_mock_server() -> SocketAddr {
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
        _ => Response::builder()
            .status(404)
            .body(Body::from("Not Found"))
            .unwrap()
    };
    Ok(response)
} 