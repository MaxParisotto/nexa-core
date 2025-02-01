use thiserror::Error;

#[derive(Debug, thiserror::Error)]
pub enum NexaError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Invalid configuration: {0}")]
    Config(String),
    
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, NexaError>;

impl NexaError {
    pub fn protocol(msg: impl Into<String>) -> Self {
        NexaError::Protocol(msg.into())
    }

    pub fn agent(msg: impl Into<String>) -> Self {
        NexaError::Agent(msg.into())
    }

    pub fn system(msg: impl Into<String>) -> Self {
        NexaError::System(msg.into())
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for NexaError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        NexaError::protocol(error.to_string())
    }
}
