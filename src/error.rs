use thiserror::Error;

#[derive(Debug, Error)]
pub enum NexaError {
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Agent error: {0}")]
    Agent(String),
    
    #[error("System error: {0}")]
    System(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
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
