use thiserror::Error;
use std::io;
use tokio_tungstenite::tungstenite;

#[derive(Debug, Error)]
pub enum NexaError {
    #[error("System error: {0}")]
    System(String),
    
    #[error("Server error: {0}")]
    Server(String),
    
    #[error("Memory error: {0}")]
    Memory(String),
    
    #[error("Token error: {0}")]
    Token(String),
    
    #[error("Monitoring error: {0}")]
    Monitoring(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Cluster error: {0}")]
    Cluster(String),
    
    #[error("Agent error: {0}")]
    Agent(String),
    
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tungstenite::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    
    #[error("Control error: {0}")]
    Control(#[from] ctrlc::Error),
}

impl From<&str> for NexaError {
    fn from(s: &str) -> Self {
        NexaError::System(s.to_string())
    }
}

impl From<String> for NexaError {
    fn from(s: String) -> Self {
        NexaError::System(s)
    }
} 