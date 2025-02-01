#[derive(Debug, thiserror::Error)]
pub enum NexaError {
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Agent error: {0}")]
    Agent(String),
    
    #[error("System error: {0}")]
    System(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("YAML error: {0}")]
    Yaml(String),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl NexaError {
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }

    pub fn agent(msg: impl Into<String>) -> Self {
        Self::Agent(msg.into())
    }

    pub fn system(msg: impl Into<String>) -> Self {
        Self::System(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::ConfigError(msg.into())
    }

    pub fn yaml(msg: impl Into<String>) -> Self {
        Self::Yaml(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, NexaError>; 