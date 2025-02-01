use std::error::Error as StdError;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum NexaError {
    Protocol(String),
    Agent(String),
    System(String),
}

impl fmt::Display for NexaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NexaError::Protocol(msg) => write!(f, "Protocol error: {}", msg),
            NexaError::Agent(msg) => write!(f, "Agent error: {}", msg),
            NexaError::System(msg) => write!(f, "System error: {}", msg),
        }
    }
}

impl StdError for NexaError {}

pub type Result<T> = std::result::Result<T, NexaError>;

impl NexaError {
    pub fn protocol(msg: impl Into<String>) -> Self {
        NexaError::Protocol(msg.into())
    }

    pub fn agent(msg: impl Into<String>) -> Self {
        NexaError::Agent(msg.into())
    }

    pub fn system<S: Into<String>>(message: S) -> Self {
        NexaError::System(message.into())
    }
}

impl From<io::Error> for NexaError {
    fn from(error: io::Error) -> Self {
        NexaError::system(error.to_string())
    }
}

impl From<serde_json::Error> for NexaError {
    fn from(error: serde_json::Error) -> Self {
        NexaError::system(error.to_string())
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for NexaError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        NexaError::system(error.to_string())
    }
}
