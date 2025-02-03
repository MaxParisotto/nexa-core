use thiserror::Error;

#[derive(Debug, Error)]
pub enum NexaError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("System error: {0}")]
    System(String),
    
    #[error("Server error: {0}")]
    Server(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Cluster error: {0}")]
    Cluster(String),

    #[error("Agent error: {0}")]
    Agent(String),
}

impl From<serde_yaml::Error> for NexaError {
    fn from(err: serde_yaml::Error) -> Self {
        NexaError::Config(format!("YAML error: {}", err))
    }
}

impl From<String> for NexaError {
    fn from(s: String) -> Self {
        NexaError::System(s)
    }
}

impl From<&str> for NexaError {
    fn from(s: &str) -> Self {
        NexaError::System(s.to_string())
    }
}

pub type Result<T> = std::result::Result<T, NexaError>; 