use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use crate::cli::{LLMModel};

#[derive(Debug, Clone)]
pub struct NexaApp {
    pub handler: Arc<crate::cli::CliHandler>,
    pub server_status: String,
    pub total_connections: u64,
    pub active_connections: u32,
    pub failed_connections: u64,
    pub last_error: Option<String>,
    pub uptime: Duration,
    pub should_exit: bool,
    pub current_view: View,
    pub server_logs: Vec<String>,
    pub error_logs: Vec<String>,
}

impl NexaApp {
    pub fn new(handler: std::sync::Arc<crate::cli::CliHandler>) -> Self {
        Self {
            handler,
            server_status: "Stopped".to_string(),
            total_connections: 0,
            active_connections: 0,
            failed_connections: 0,
            last_error: None,
            uptime: std::time::Duration::from_secs(0),
            should_exit: false,
            current_view: View::Overview,
            server_logs: Vec::new(),
            error_logs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    UpdateState(String, usize),
    StartServer,
    StopServer,
    ServerStarted(bool, Option<String>),
    ServerStopped(bool, Option<String>),
    Exit,
    ChangeView(View),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum View {
    Overview,
    Agents,
    Tasks,
    Connections,
    Settings,
    LLMServers,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskPriority::Low => write!(f, "Low"),
            TaskPriority::Normal => write!(f, "Normal"),
            TaskPriority::High => write!(f, "High"),
            TaskPriority::Critical => write!(f, "Critical"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub level: LogLevel,
    pub source: String,
}

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Info,
    Error,
    Debug,
    Warning,
}

#[derive(Debug, Clone)]
pub struct LLMServer {
    pub provider: String,
    pub address: String,
    pub status: LLMStatus,
    pub last_error: Option<String>,
    pub available_models: Vec<LLMModel>,
    pub model_names: Vec<String>,
    pub selected_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LLMStatus {
    Connected,
    Disconnected,
    Connecting,
    Error,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CustomPrompt {
    pub name: String,
    pub template: String,
    pub parameters: Vec<String>,
} 