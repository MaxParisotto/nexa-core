#![allow(dead_code, unused_imports, unused_variables)]

use thiserror::Error;
use std::io;
use tokio_tungstenite::tungstenite;
use std::error::Error as StdError;
use reqwest;

/// Custom error types for the Nexa system
#[derive(Debug, Error, Clone)]
pub enum NexaError {
    /// System-level errors
    #[error("System error: {0}")]
    System(String),
    
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),
    
    /// I/O errors
    #[error("IO error: {0}")]
    Io(String),
    
    /// JSON serialization/deserialization errors
    #[error("JSON error: {0}")]
    Json(String),
    
    /// Server errors
    #[error("Server error: {0}")]
    Server(String),
    
    /// Protocol errors
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    /// WebSocket errors
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    /// YAML errors
    #[error("YAML error: {0}")]
    Yaml(String),
    
    /// Control errors
    #[error("Control error: {0}")]
    Control(String),
    
    /// Agent-related errors
    #[error("Agent error: {0}")]
    Agent(String),
    
    /// LLM-related errors
    #[error("LLM error: {0}")]
    LLMError(String),
    
    /// LLM connection errors
    #[error("LLM connection error: {0}")]
    LLMConnection(String),
    
    /// LLM response parsing errors
    #[error("LLM response error: {0}")]
    LLMResponse(String),
    
    /// LLM rate limit errors
    #[error("LLM rate limit error: {0}")]
    LLMRateLimit(String),
    
    /// LLM token limit errors
    #[error("LLM token limit error: {0}")]
    LLMTokenLimit(String),
    
    /// Cluster errors
    #[error("Cluster error: {0}")]
    Cluster(String),
    
    /// Memory allocation errors
    #[error("Memory error: {0}")]
    Memory(String),
    
    /// Resource errors
    #[error("Resource error: {0}")]
    Resource(String),
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
        match e {
            tungstenite::Error::Protocol(msg) => NexaError::Protocol(msg.to_string()),
            tungstenite::Error::Io(e) => NexaError::Io(e.to_string()),
            tungstenite::Error::ConnectionClosed => NexaError::WebSocket("Connection closed".to_string()),
            _ => NexaError::WebSocket(e.to_string()),
        }
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

impl From<reqwest::Error> for NexaError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            NexaError::LLMResponse("Request timed out".to_string())
        } else if err.is_connect() {
            NexaError::LLMConnection("Failed to connect to LLM server".to_string())
        } else if err.is_body() {
            NexaError::LLMResponse("Invalid response body".to_string())
        } else if err.is_decode() {
            NexaError::LLMResponse("Failed to decode response".to_string())
        } else {
            NexaError::LLMError(err.to_string())
        }
    }
}

impl From<Box<dyn StdError>> for NexaError {
    fn from(err: Box<dyn StdError>) -> Self {
        if let Some(e) = err.downcast_ref::<tungstenite::Error>() {
            match e {
                tungstenite::Error::Protocol(msg) => NexaError::Protocol(msg.to_string()),
                tungstenite::Error::Io(e) => NexaError::Io(e.to_string()),
                tungstenite::Error::ConnectionClosed => NexaError::WebSocket("Connection closed".to_string()),
                _ => NexaError::WebSocket(e.to_string()),
            }
        } else if let Some(e) = err.downcast_ref::<io::Error>() {
            NexaError::Io(e.to_string())
        } else if let Some(e) = err.downcast_ref::<reqwest::Error>() {
            if e.is_timeout() {
                NexaError::LLMResponse("Request timed out".to_string())
            } else if e.is_connect() {
                NexaError::LLMConnection("Failed to connect to LLM server".to_string())
            } else if e.is_body() {
                NexaError::LLMResponse("Invalid response body".to_string())
            } else if e.is_decode() {
                NexaError::LLMResponse("Failed to decode response".to_string())
            } else {
                NexaError::LLMError(e.to_string())
            }
        } else {
            NexaError::System(err.to_string())
        }
    }
} 