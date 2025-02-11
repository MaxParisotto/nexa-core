#![allow(dead_code, unused_imports, unused_variables)]

use tokio::net::{TcpListener, TcpStream};
use crate::error::NexaError;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{accept_async, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;
use log::{debug, error};

pub fn hello_world() -> &'static str {
    "Hello from nexa-utils!"
}

/// Create a WebSocket server
pub async fn create_ws_server(addr: &str) -> Result<TcpListener, NexaError> {
    let listener = TcpListener::bind(addr).await?;
    Ok(listener)
}

pub async fn handle_ws_connection(stream: TcpStream) -> Result<(), NexaError> {
    let ws_stream = accept_async(stream)
        .await
        .map_err(|e| NexaError::System(e.to_string()))?;
    
    debug!("New WebSocket connection established");
    handle_ws_messages(ws_stream).await
}

async fn handle_ws_messages(mut ws_stream: WebSocketStream<TcpStream>) -> Result<(), NexaError> {
    while let Some(msg) = ws_stream.next().await {
        match msg {
            Ok(msg) => {
                match msg {
                    Message::Text(text) => {
                        debug!("Received text message: {}", text);
                        // Echo the message back with a success response
                        let response = serde_json::json!({
                            "code": 200,
                            "message": "Message received"
                        });
                            ws_stream.send(Message::Text(response.to_string().into()))
                            .await
                            .map_err(|e| NexaError::System(e.to_string()))?;
                        debug!("Client initiated close");
                        break;
                    }
                    _ => {
                        debug!("Received non-text message");
                        // Send error response for unsupported message types
                        let response = serde_json::json!({
                            "code": 400,
                            "message": "Unsupported message type"
                        });
                            ws_stream.send(Message::Text(response.to_string().into()))
                            .await
                            .map_err(|e| NexaError::System(e.to_string()))?;
                    }
                }
            }
            Err(e) => {
                error!("Error receiving message: {}", e);
                return Err(NexaError::System(e.to_string()));
            }
        }
    }
    
    debug!("WebSocket connection closed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world() {
        assert_eq!(hello_world(), "Hello from nexa-utils!");
    }

    #[tokio::test]
    async fn test_ws_server_creation() {
        let result = create_ws_server("127.0.0.1:0").await;
        assert!(result.is_ok());
    }
}
