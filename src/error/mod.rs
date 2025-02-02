#[derive(Debug, thiserror::Error)]
pub enum NexaError {
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Agent error: {0}")]
    Agent(String),
    
    #[error("System error: {0}")]
    System(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("YAML error: {0}")]
    Yaml(String),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Cluster error: {0}")]
    Cluster(String),

    #[error("Server error: {0}")]
    Server(String),

    #[error("Signal handler error: {0}")]
    Signal(String),
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
        Self::Config(msg.into())
    }

    pub fn yaml(msg: impl Into<String>) -> Self {
        Self::Yaml(msg.into())
    }

    pub fn cluster(msg: impl Into<String>) -> Self {
        Self::Cluster(msg.into())
    }

    pub fn server<S: Into<String>>(msg: S) -> Self {
        Self::Server(msg.into())
    }

    pub fn signal<S: Into<String>>(msg: S) -> Self {
        Self::Signal(msg.into())
    }
}

impl From<ctrlc::Error> for NexaError {
    fn from(err: ctrlc::Error) -> Self {
        Self::Signal(format!("Signal handler error: {}", err))
    }
}

pub type Result<T> = std::result::Result<T, NexaError>; 