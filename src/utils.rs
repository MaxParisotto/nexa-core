use tokio::net::{TcpListener, TcpStream};
use crate::error::NexaError;

pub fn hello_world() -> &'static str {
    "Hello from nexa-utils!"
}

pub async fn create_ws_server(addr: &str) -> Result<TcpListener, NexaError> {
    TcpListener::bind(addr).await.map_err(|e| NexaError::system(&e.to_string()))
}

pub async fn handle_ws_connection(stream: TcpStream) -> Result<(), NexaError> {
    let _ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(|e| NexaError::system(&e.to_string()))?;
    
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
        let server = create_ws_server("127.0.0.1:0").await;
        assert!(server.is_ok());
    }
}
