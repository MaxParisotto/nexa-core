use thiserror::Error;
use std::io;
use tokio_tungstenite::tungstenite;

#[derive(Debug, Error, Clone)]
pub enum NexaError {
    /// Represents a system-level error
    #[error("System error: {0}")]
    System(String),
    
    #[error("Server error: {0}")]
    Server(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Cluster error: {0}")]
    Cluster(String),
    
    #[error("Agent error: {0}")]
    Agent(String),
    
    #[error("IO error: {0}")]
    Io(String),
    
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    #[error("JSON error: {0}")]
    Json(String),
    
    #[error("YAML error: {0}")]
    Yaml(String),
    
    #[error("Control error: {0}")]
    Control(String),
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

impl From<io::Error> for NexaError {
    fn from(e: io::Error) -> Self {
        NexaError::Io(e.to_string())
    }
}

impl From<tungstenite::Error> for NexaError {
    fn from(e: tungstenite::Error) -> Self {
        NexaError::WebSocket(e.to_string())
    }
}

impl From<serde_json::Error> for NexaError {
    fn from(e: serde_json::Error) -> Self {
        NexaError::Json(e.to_string())
    }
}

impl From<serde_yaml::Error> for NexaError {
    fn from(e: serde_yaml::Error) -> Self {
        NexaError::Yaml(e.to_string())
    }
}

impl From<ctrlc::Error> for NexaError {
    fn from(e: ctrlc::Error) -> Self {
        NexaError::Control(e.to_string())
    }
} 