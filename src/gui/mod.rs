use iced::{Element, Length, Theme, Color, Subscription, Command};
use iced::executor;
use iced::window;
use iced::widget::{row, text, container, scrollable, Column, Button, TextInput, PickList, Rule, Row, Checkbox};
use iced::{Application, Settings};
use std::sync::Arc;
use std::time::Duration;
use std::collections::VecDeque;
use crate::server::ServerMetrics;
use crate::error::NexaError;
use crate::cli::{CliHandler, LLMModel, AgentConfig, RetryPolicy, WorkflowStep, AgentWorkflow, Agent as CliAgent};
use log::info;
use chrono::Utc;
use std::ops::Deref;
use crate::types::agent::{Task, TaskStatus};
use iced::widget::container::StyleSheet;
use serde_json;
use sysinfo;

#[derive(Debug, Clone)]
struct NavStyle;

impl container::StyleSheet for NavStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(iced::Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
            border_radius: 4.0.into(),
            border_width: 1.0,
            border_color: Color::from_rgb(0.8, 0.8, 0.8),
            ..Default::default()
        }
    }
}

impl From<NavStyle> for iced::theme::Container {
    fn from(_: NavStyle) -> Self {
        iced::theme::Container::Custom(Box::new(NavStyle))
    }
}

const MAX_LOG_ENTRIES: usize = 100;

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    UpdateState(String, usize, ServerMetrics),
    StartServer,
    StopServer,
    ServerStarted(bool, Option<String>),
    ServerStopped(bool, Option<String>),
    Exit,
    // Agent management
    CreateAgent,
    AgentNameChanged(String),
    AgentCapabilitiesChanged(String),
    AgentCreated(Result<CliAgent, String>),
    // Task management
    CreateTask,
    TaskDescriptionChanged(String),
    TaskPriorityChanged(TaskPriority),
    TaskAssignedAgent(String),
    TaskCreated(Result<(), String>),
    // Connection management
    SetMaxConnections(String),
    MaxConnectionsUpdated(Result<(), String>),
    // View management
    ChangeView(View),
    /// Message for LLM connection result: (agent_id, provider, result)
    LLMConnected(String, String, Result<(), String>),
    // LLM Server management
    AddLLMServer(String),  // provider name
    RemoveLLMServer(String),  // provider name
    LLMAddressChanged(String),
    LLMProviderChanged(String),
    ConnectLLM(String),  // provider name
    DisconnectLLM(String),  // provider name
    ModelsLoaded(String, Result<Vec<LLMModel>, String>),
    SelectModel(String, String),  // (provider, model)
    ModelSelected(String, String, Result<(), String>),  // (provider, model, result)
    RefreshModels(String),  // provider name
    TestModel(String, String),  // (provider, model)
    ModelTested(String, String, Result<String, String>),  // (provider, model, result)
    CreateNewAgent(String, AgentConfig),
    CreateNewWorkflow(String, Vec<WorkflowStep>),
    WorkflowCreated(Result<AgentWorkflow, String>),
    ExecuteWorkflow(String),
    WorkflowExecuted(Result<(), String>),
    UpdateAgentCapabilities(String, Vec<String>),
    CapabilitiesUpdated(Result<(), String>),
    SetAgentHierarchy(String, String),
    HierarchyUpdated(Result<(), String>),
    MaxConcurrentTasksChanged(String),
    PriorityThresholdChanged(String),
    MaxRetriesChanged(String),
    BackoffMsChanged(String),
    MaxBackoffMsChanged(String),
    TimeoutSecondsChanged(String),
    TestAgent(String),
    ToggleAgentStatus(String),
    DeleteAgent(String),
    AgentTested,
    AgentStatusToggled,
    AgentDeleted,
    AgentsLoaded(Vec<CliAgent>),
    Error(String),
    ToggleMCPLocalFiles(bool),
    ToggleMCPDatabases(bool),
    ToggleMCPAPIs(bool),
    ToggleMCPCodeAnalysis(bool),
    ToggleMCPTextProcessing(bool),
    ToggleMCPDataTransform(bool),
    MCPCustomPromptsChanged(String),
    ToggleMCPRootAccess(bool),
    MCPTransportChanged(String),
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
struct LogEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    message: String,
    level: LogLevel,
    source: String,
}

#[derive(Debug, Clone, Copy)]
enum LogLevel {
    Info,
    Error,
    Debug,
    Warning,
}

#[derive(Debug, Clone)]
struct LLMServer {
    provider: String,
    address: String,
    status: LLMStatus,
    last_error: Option<String>,
    available_models: Vec<LLMModel>,
    model_names: Vec<String>,
    selected_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum LLMStatus {
    Connected,
    Disconnected,
    Connecting,
    Error,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CustomPrompt {
    pub name: String,
    pub template: String,
    pub parameters: Vec<String>,
}

pub struct NexaApp {
    handler: Arc<CliHandler>,
    server_status: String,
    total_connections: u64,
    active_connections: u32,
    failed_connections: u64,
    last_error: Option<String>,
    uptime: Duration,
    should_exit: bool,
    server_logs: VecDeque<LogEntry>,
    connection_logs: VecDeque<LogEntry>,
    error_logs: VecDeque<LogEntry>,
    // Agent management
    new_agent_name: String,
    new_agent_capabilities: String,
    agents: Vec<CliAgent>,
    agent_options: Vec<String>,
    // Task management
    new_task_description: String,
    new_task_priority: TaskPriority,
    selected_agent: Option<String>,
    tasks: Vec<Task>,
    // Connection management
    max_connections_input: String,
    // View management
    current_view: View,
    llm_servers: Vec<LLMServer>,
    new_llm_address: String,
    new_llm_provider: String,
    empty_models: Vec<String>,
    selected_provider: String,
    selected_model: String,
    max_concurrent_tasks_input: String,
    priority_threshold_input: String,
    max_retries_input: String,
    backoff_ms_input: String,
    max_backoff_ms_input: String,
    timeout_seconds_input: String,
    mcp_local_files: bool,
    mcp_databases: bool,
    mcp_apis: bool,
    mcp_code_analysis: bool,
    mcp_text_processing: bool,
    mcp_data_transform: bool,
    mcp_custom_prompts: String,
    mcp_root_access: bool,
    mcp_transport: String,
}

impl Default for NexaApp {
    fn default() -> Self {
        Self {
            handler: Arc::new(CliHandler::new()),
            server_status: "Stopped".to_string(),
            total_connections: 0,
            active_connections: 0,
            failed_connections: 0,
            last_error: None,
            uptime: Duration::from_secs(0),
            should_exit: false,
            server_logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            connection_logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            error_logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            new_agent_name: String::new(),
            new_agent_capabilities: String::new(),
            agents: Vec::new(),
            agent_options: Vec::new(),
            new_task_description: String::new(),
            new_task_priority: TaskPriority::Normal,
            selected_agent: None,
            tasks: Vec::new(),
            max_connections_input: String::new(),
            current_view: View::Overview,
            llm_servers: vec![
                LLMServer {
                    provider: "LMStudio".to_string(),
                    address: "http://localhost:1234".to_string(),
                    status: LLMStatus::Disconnected,
                    last_error: None,
                    available_models: Vec::new(),
                    model_names: Vec::new(),
                    selected_model: None,
                },
                LLMServer {
                    provider: "Ollama".to_string(),
                    address: "http://localhost:11434".to_string(),
                    status: LLMStatus::Disconnected,
                    last_error: None,
                    available_models: Vec::new(),
                    model_names: Vec::new(),
                    selected_model: None,
                },
            ],
            new_llm_address: String::new(),
            new_llm_provider: String::new(),
            empty_models: Vec::new(),
            selected_provider: String::new(),
            selected_model: String::new(),
            max_concurrent_tasks_input: "5".to_string(),
            priority_threshold_input: "2".to_string(),
            max_retries_input: "3".to_string(),
            backoff_ms_input: "1000".to_string(),
            max_backoff_ms_input: "10000".to_string(),
            timeout_seconds_input: "30".to_string(),
            mcp_local_files: false,
            mcp_databases: false,
            mcp_apis: false,
            mcp_code_analysis: false,
            mcp_text_processing: false,
            mcp_data_transform: false,
            mcp_custom_prompts: String::new(),
            mcp_root_access: false,
            mcp_transport: "WebSocket".to_string(),
        }
    }
}

impl NexaApp {
    pub fn new(handler: Arc<CliHandler>) -> Self {
        info!("Creating new NexaApp instance");
        Self {
            handler,
            server_status: "Stopped".to_string(),
            total_connections: 0,
            active_connections: 0,
            failed_connections: 0,
            last_error: None,
            uptime: Duration::from_secs(0),
            should_exit: false,
            server_logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            connection_logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            error_logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            new_agent_name: String::new(),
            new_agent_capabilities: String::new(),
            agents: Vec::new(),
            agent_options: Vec::new(),
            new_task_description: String::new(),
            new_task_priority: TaskPriority::Normal,
            selected_agent: None,
            tasks: Vec::new(),
            max_connections_input: String::new(),
            current_view: View::Overview,
            llm_servers: vec![
                LLMServer {
                    provider: "LMStudio".to_string(),
                    address: "http://localhost:1234".to_string(),
                    status: LLMStatus::Disconnected,
                    last_error: None,
                    available_models: Vec::new(),
                    model_names: Vec::new(),
                    selected_model: None,
                },
                LLMServer {
                    provider: "Ollama".to_string(),
                    address: "http://localhost:11434".to_string(),
                    status: LLMStatus::Disconnected,
                    last_error: None,
                    available_models: Vec::new(),
                    model_names: Vec::new(),
                    selected_model: None,
                },
            ],
            new_llm_address: String::new(),
            new_llm_provider: String::new(),
            empty_models: Vec::new(),
            selected_provider: String::new(),
            selected_model: String::new(),
            max_concurrent_tasks_input: "5".to_string(),
            priority_threshold_input: "2".to_string(),
            max_retries_input: "3".to_string(),
            backoff_ms_input: "1000".to_string(),
            max_backoff_ms_input: "10000".to_string(),
            timeout_seconds_input: "30".to_string(),
            mcp_local_files: false,
            mcp_databases: false,
            mcp_apis: false,
            mcp_code_analysis: false,
            mcp_text_processing: false,
            mcp_data_transform: false,
            mcp_custom_prompts: String::new(),
            mcp_root_access: false,
            mcp_transport: "WebSocket".to_string(),
        }
    }

    fn add_log(&mut self, message: impl Into<String>, level: LogLevel, log_type: &str) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            message: message.into(),
            level,
            source: log_type.to_string(),
        };

        let logs = match log_type {
            "server" => &mut self.server_logs,
            "connection" => &mut self.connection_logs,
            "error" => &mut self.error_logs,
            _ => return,
        };

        if logs.len() >= MAX_LOG_ENTRIES {
            logs.pop_front();
        }
        logs.push_back(entry);
    }

    fn view_log_section(&self, title: &str, logs: &VecDeque<LogEntry>) -> Element<Message> {
        let header = row![
            text(title).size(20).width(Length::Fill),
            text(format!("{} entries", logs.len())).size(14).style(Color::from_rgb(0.5, 0.5, 0.5))
        ].padding(5);

        let mut log_content = Column::new()
            .spacing(8)
            .push(header)
            .push(Rule::horizontal(2));

        for entry in logs.iter() {
            let (color, level_text) = match entry.level {
                LogLevel::Info => (Color::from_rgb(0.0, 0.7, 0.0), "INFO"),
                LogLevel::Error => (Color::from_rgb(0.8, 0.0, 0.0), "ERROR"),
                LogLevel::Debug => (Color::from_rgb(0.5, 0.5, 0.5), "DEBUG"),
                LogLevel::Warning => (Color::from_rgb(0.8, 0.6, 0.0), "WARN"),
            };

            let log_row = row![
                text(entry.timestamp.format("%H:%M:%S%.3f")).size(12).style(Color::from_rgb(0.4, 0.4, 0.4)).width(Length::Fixed(100.0)),
                text(level_text).size(12).style(color).width(Length::Fixed(50.0)),
                text(&entry.source).size(12).style(Color::from_rgb(0.3, 0.5, 0.7)).width(Length::Fixed(80.0)),
                text(&entry.message).size(14).style(color)
            ].spacing(10);

            log_content = log_content.push(
                container(log_row)
                    .padding(5)
                    .style(if matches!(entry.level, LogLevel::Error) {
                        iced::theme::Container::Custom(Box::new(ErrorLogStyle))
                    } else {
                        iced::theme::Container::Transparent
                    })
            );
        }

        scrollable(
            container(log_content)
                .width(Length::Fill)
                .padding(10)
                .style(iced::theme::Container::Box)
        )
        .height(Length::Fixed(300.0))
        .into()
    }

    fn view_agents(&self) -> Element<Message> {
        let content = Column::new()
            .spacing(20)
            .push(text("Agent Management").size(24));

        // Create New Agent Section
        let new_agent_section = Column::new()
            .spacing(10)
            .push(text("Create New Agent").size(20))
            .push(
                Column::new()
                    .spacing(20)
                    .push(
                        container(
                            Column::new()
                                .spacing(10)
                                .push(text("Basic Settings").size(16))
                                .push(
                                    Row::new()
                                        .spacing(10)
                                        .push(text("Name:").width(Length::Fixed(120.0)))
                                        .push(
                                            TextInput::new("Enter agent name...", &self.new_agent_name)
                                                .on_input(Message::AgentNameChanged)
                                                .padding(5)
                                                .width(Length::Fixed(300.0))
                                        )
                                )
                        )
                        .padding(10)
                        .style(iced::theme::Container::Box)
                    )
                    .push(
                        container(
                            Column::new()
                                .spacing(10)
                                .push(text("MCP Configuration").size(16))
                                .push(
                                    Row::new()
                                        .spacing(10)
                                        .push(text("Data Sources:").width(Length::Fixed(120.0)))
                                        .push(
                                            Column::new()
                                                .spacing(5)
                                                .push(
                                                    Checkbox::new(
                                                        "Local Files",
                                                        self.mcp_local_files,
                                                        |checked| Message::ToggleMCPLocalFiles(checked)
                                                    )
                                                )
                                                .push(
                                                    Checkbox::new(
                                                        "Databases",
                                                        self.mcp_databases,
                                                        |checked| Message::ToggleMCPDatabases(checked)
                                                    )
                                                )
                                                .push(
                                                    Checkbox::new(
                                                        "APIs",
                                                        self.mcp_apis,
                                                        |checked| Message::ToggleMCPAPIs(checked)
                                                    )
                                                )
                                        )
                                )
                                .push(
                                    Row::new()
                                        .spacing(10)
                                        .push(text("Tools:").width(Length::Fixed(120.0)))
                                        .push(
                                            Column::new()
                                                .spacing(5)
                                                .push(
                                                    Checkbox::new(
                                                        "Code Analysis",
                                                        self.mcp_code_analysis,
                                                        |checked| Message::ToggleMCPCodeAnalysis(checked)
                                                    )
                                                )
                                                .push(
                                                    Checkbox::new(
                                                        "Text Processing",
                                                        self.mcp_text_processing,
                                                        |checked| Message::ToggleMCPTextProcessing(checked)
                                                    )
                                                )
                                                .push(
                                                    Checkbox::new(
                                                        "Data Transformation",
                                                        self.mcp_data_transform,
                                                        |checked| Message::ToggleMCPDataTransform(checked)
                                                    )
                                                )
                                        )
                                )
                                .push(
                                    Row::new()
                                        .spacing(10)
                                        .push(text("Custom Prompts:").width(Length::Fixed(120.0)))
                                        .push(
                                            TextInput::new("Enter custom prompts (JSON)...", &self.mcp_custom_prompts)
                                                .on_input(Message::MCPCustomPromptsChanged)
                                                .padding(5)
                                                .width(Length::Fixed(300.0))
                                        )
                                )
                                .push(
                                    Row::new()
                                        .spacing(10)
                                        .push(text("Root Access:").width(Length::Fixed(120.0)))
                                        .push(
                                            Checkbox::new(
                                                "Allow root access",
                                                self.mcp_root_access,
                                                |checked| Message::ToggleMCPRootAccess(checked)
                                            )
                                        )
                                )
                                .push(
                                    Row::new()
                                        .spacing(10)
                                        .push(text("Transport:").width(Length::Fixed(120.0)))
                                        .push(
                                            PickList::new(
                                                vec!["WebSocket".to_string(), "gRPC".to_string(), "HTTP/2".to_string()],
                                                Some(self.mcp_transport.clone()),
                                                Message::MCPTransportChanged
                                            )
                                            .width(Length::Fixed(200.0))
                                        )
                                )
                        )
                        .padding(10)
                        .style(iced::theme::Container::Box)
                    )
                    .push(
                        container(
                            Column::new()
                                .spacing(10)
                                .push(text("Capabilities:").width(Length::Fixed(120.0)))
                                .push(
                                    TextInput::new("Enter capabilities (comma-separated)...", &self.new_agent_capabilities)
                                        .on_input(Message::AgentCapabilitiesChanged)
                                        .padding(5)
                                        .width(Length::Fixed(300.0))
                                )
                        )
                        .padding(10)
                        .style(iced::theme::Container::Box)
                    )
            );

        let content = content.push(
            container(new_agent_section)
                .width(Length::Fill)
                .padding(10)
                .style(iced::theme::Container::Box)
        );

        let scrollable_content = scrollable(content)
            .height(Length::Fill);

        scrollable_content.into()
    }

    fn view_tasks(&self) -> Element<Message> {
        let mut content = Column::new()
            .spacing(20)
            .push(text("Task Management").size(24))
            .push(
                Column::new()
                    .spacing(10)
                    .push(text("Create New Task"))
                    .push(
                        TextInput::new("Enter task description...", &self.new_task_description)
                            .on_input(Message::TaskDescriptionChanged)
                    )
                    .push(
                        PickList::new(
                            &[TaskPriority::Low, TaskPriority::Normal, TaskPriority::High, TaskPriority::Critical][..],
                            Some(self.new_task_priority),
                            Message::TaskPriorityChanged,
                        )
                    )
                    .push(
                        PickList::new(
                            &self.agent_options,
                            self.selected_agent.clone(),
                            Message::TaskAssignedAgent,
                        )
                    )
                    .push(
                        Button::new("Create Task")
                            .on_press(Message::CreateTask)
                    )
            );

        // List existing tasks
        content = content.push(Rule::horizontal(10))
            .push(text("Active Tasks").size(20));

        for task in &self.tasks {
            content = content.push(
                container(
                    Column::new()
                        .spacing(5)
                        .push(text(&format!("Description: {}", task.description)))
                        .push(text(&format!("Priority: {}", task.priority)))
                        .push(text(&format!("Status: {:?}", task.status)))
                        .push(text(&format!("Assigned to: {}", task.assigned_agent.as_deref().unwrap_or("N/A"))))
                        .push(text(&format!("Created: {}", task.created_at.format("%Y-%m-%d %H:%M:%S"))))
                        .push(
                            if let Some(deadline) = task.deadline {
                                text(&format!("Deadline: {}", deadline.format("%Y-%m-%d %H:%M:%S")))
                            } else {
                                text("No deadline set")
                            }
                        )
                )
                .padding(10)
                .style(iced::theme::Container::Box)
            );
        }

        scrollable(content).height(Length::Fill).into()
    }

    fn view_connections(&self) -> Element<Message> {
        Column::new()
            .spacing(20)
            .push(text("Connection Management").size(24))
            .push(
                Column::new()
                    .spacing(10)
                    .push(text("Connection Settings"))
                    .push(
                        TextInput::new("Enter max connections...", &self.max_connections_input)
                            .on_input(Message::SetMaxConnections)
                    )
            )
            .push(Rule::horizontal(10))
            .push(
                Column::new()
                    .spacing(5)
                    .push(text(&format!("Active Connections: {}", self.active_connections)))
                    .push(text(&format!("Total Connections: {}", self.total_connections)))
                    .push(text(&format!("Failed Connections: {}", self.failed_connections)))
            )
            .push(Rule::horizontal(10))
            .push(self.view_log_section("Connection Logs", &self.connection_logs))
            .into()
    }

    fn view_settings(&self) -> Element<Message> {
        Column::new()
            .spacing(20)
            .push(text("Server Settings").size(24))
            .push(
                row![
                    if self.server_status == "Running" {
                        Button::new("Stop Server")
                            .on_press(Message::StopServer)
                    } else {
                        Button::new("Start Server")
                            .on_press(Message::StartServer)
                    },
                    Button::new("Exit")
                        .on_press(Message::Exit)
                ].spacing(20)
            )
            .push(Rule::horizontal(10))
            .push(self.view_log_section("Server Logs", &self.server_logs))
            .into()
    }

    fn view_llm_servers(&self) -> Element<Message> {
        let mut content = Column::new()
            .spacing(20)
            .push(text("LLM Server Management").size(24));

        // Add new LLM server section
        content = content.push(
            Column::new()
                .spacing(10)
                .push(text("Add New LLM Server"))
                .push(
                    TextInput::new("Provider name...", &self.new_llm_provider)
                        .on_input(Message::LLMProviderChanged)
                        .padding(10)
                )
                .push(
                    TextInput::new("Server address...", &self.new_llm_address)
                        .on_input(Message::LLMAddressChanged)
                        .padding(10)
                )
                .push(
                    Button::new("Add Server")
                        .on_press(Message::AddLLMServer(self.new_llm_provider.clone()))
                        .padding(10)
                )
        );

        // List existing LLM servers
        content = content.push(Rule::horizontal(10))
            .push(text("Active LLM Servers").size(20));

        for server in &self.llm_servers {
            let status_color = match server.status {
                LLMStatus::Connected => Color::from_rgb(0.0, 0.7, 0.0),
                LLMStatus::Disconnected => Color::from_rgb(0.5, 0.5, 0.5),
                LLMStatus::Connecting => Color::from_rgb(0.7, 0.7, 0.0),
                LLMStatus::Error => Color::from_rgb(0.7, 0.0, 0.0),
            };

            let model_picker = if !server.available_models.is_empty() {
                PickList::new(
                    &server.model_names,
                    server.selected_model.clone(),
                    move |model| Message::SelectModel(server.provider.clone(), model)
                )
                .width(Length::Fixed(200.0))
            } else {
                PickList::new(
                    &self.empty_models,
                    None,
                    |_| Message::SelectModel(server.provider.clone(), String::new())
                )
                .width(Length::Fixed(200.0))
                .placeholder("No models available")
            };

            let server_row = row![
                text(&server.provider).width(Length::Fixed(120.0)),
                text(&server.address).width(Length::Fixed(200.0)),
                text(format!("{:?}", server.status)).style(status_color).width(Length::Fixed(100.0)),
                model_picker,
                Button::new("Refresh Models")
                    .on_press(Message::RefreshModels(server.provider.clone())),
                if server.status == LLMStatus::Connected {
                    Button::new("Disconnect")
                        .on_press(Message::DisconnectLLM(server.provider.clone()))
                } else {
                    Button::new("Connect")
                        .on_press(Message::ConnectLLM(server.provider.clone()))
                },
                Button::new("Remove")
                    .on_press(Message::RemoveLLMServer(server.provider.clone()))
                    .style(iced::theme::Button::Destructive)
            ]
            .spacing(20)
            .padding(10);

            content = content.push(server_row);

            // Show selected model details if any
            if let Some(model_name) = &server.selected_model {
                if let Some(model) = server.available_models.iter().find(|m| m.name == *model_name) {
                    let model_details = Column::new()
                        .spacing(5)
                        .push(
                            row![
                                text(format!("Selected Model: {}", model.name))
                                    .size(14)
                                    .style(Color::from_rgb(0.0, 0.5, 0.7)),
                                Button::new("Test")
                                    .on_press(Message::TestModel(server.provider.clone(), model_name.clone()))
                            ].spacing(20)
                        )
                        .push(
                            row![
                                text(format!("Size: {}", model.size)).width(Length::Fixed(150.0)),
                                text(format!("Context: {} tokens", model.context_length)).width(Length::Fixed(200.0)),
                                if let Some(quant) = &model.quantization {
                                    text(format!("Quantization: {}", quant))
                                } else {
                                    text("No quantization")
                                }
                            ].spacing(20)
                        )
                        .push(
                            text(&model.description)
                                .size(12)
                                .style(Color::from_rgb(0.5, 0.5, 0.5))
                        );

                    content = content.push(
                        container(model_details)
                            .padding(10)
                            .style(iced::theme::Container::Box)
                    );
                }
            }

            if let Some(error) = &server.last_error {
                content = content.push(
                    text(error)
                        .style(Color::from_rgb(0.7, 0.0, 0.0))
                        .size(12)
                );
            }
        }

        // Add log section
        content = content.push(Rule::horizontal(10))
            .push(self.view_log_section("LLM Connection Logs", &self.connection_logs));

        scrollable(content).into()
    }

    /// Returns a Webmin-style sidebar with navigation buttons.
    fn view_sidebar(&self) -> Element<Message> {
        let sidebar = iced::widget::Column::new()
            .spacing(20)
            .padding(20)
            .push(iced::widget::Text::new("Menu").size(28))
            .push(
                iced::widget::Button::new(iced::widget::Text::new("Overview"))
                    .on_press(Message::ChangeView(View::Overview))
                    .style(if self.current_view == View::Overview {
                        iced::theme::Button::Primary
                    } else {
                        iced::theme::Button::Text
                    })
            )
            .push(
                iced::widget::Button::new(iced::widget::Text::new("Agents"))
                    .on_press(Message::ChangeView(View::Agents))
                    .style(if self.current_view == View::Agents {
                        iced::theme::Button::Primary
                    } else {
                        iced::theme::Button::Text
                    })
            )
            .push(
                iced::widget::Button::new(iced::widget::Text::new("Tasks"))
                    .on_press(Message::ChangeView(View::Tasks))
                    .style(if self.current_view == View::Tasks {
                        iced::theme::Button::Primary
                    } else {
                        iced::theme::Button::Text
                    })
            )
            .push(
                iced::widget::Button::new(iced::widget::Text::new("Connections"))
                    .on_press(Message::ChangeView(View::Connections))
                    .style(if self.current_view == View::Connections {
                        iced::theme::Button::Primary
                    } else {
                        iced::theme::Button::Text
                    })
            )
            .push(
                iced::widget::Button::new(iced::widget::Text::new("Settings"))
                    .on_press(Message::ChangeView(View::Settings))
                    .style(if self.current_view == View::Settings {
                        iced::theme::Button::Primary
                    } else {
                        iced::theme::Button::Text
                    })
            )
            .push(
                Button::new(text("LLM Servers"))
                    .on_press(Message::ChangeView(View::LLMServers))
                    .style(if self.current_view == View::LLMServers {
                        iced::theme::Button::Primary
                    } else {
                        iced::theme::Button::Text
                    })
            );

        iced::widget::container(sidebar)
            .width(iced::Length::Fixed(200.0))
            .height(iced::Length::Fill)
            .style(iced::theme::Container::Custom(Box::new(SidebarStyle)))
            .into()
    }

    pub fn view(&self) -> iced::Element<Message> {
        // Determine the main content based on the current view
        let main_content: iced::Element<Message> = match self.current_view {
            View::Overview => {
                let status_color = match self.server_status.as_str() {
                    "Running" => iced::Color::from_rgb(0.0, 0.8, 0.0),
                    "Stopped" => iced::Color::from_rgb(0.8, 0.0, 0.0),
                    _ => iced::Color::from_rgb(0.8, 0.8, 0.0),
                };

                let mut sys_info = sysinfo::System::new_all();
                sys_info.refresh_all();
                let cpu_usage = sys_info.global_cpu_info().cpu_usage();
                let total_memory = sys_info.total_memory() as f64 / 1024.0 / 1024.0; // Convert to MB
                let used_memory = sys_info.used_memory() as f64 / 1024.0 / 1024.0;
                let memory_usage = (used_memory / total_memory * 100.0) as f64;

                iced::widget::Column::new()
                    .spacing(20)
                    .push(
                        container(
                            Row::new()
                                .spacing(20)
                                .push(
                                    container(
                                        Column::new()
                                            .spacing(10)
                                            .push(text("Server Status").size(18))
                                            .push(
                                                text(&self.server_status)
                                                    .size(24)
                                                    .style(status_color)
                                            )
                                    )
                                    .padding(20)
                                    .style(iced::theme::Container::Box)
                                    .width(Length::Fixed(200.0))
                                )
                                .push(
                                    container(
                                        Column::new()
                                            .spacing(10)
                                            .push(text("Uptime").size(18))
                                            .push(
                                                text(format!("{:02}:{:02}:{:02}", 
                                                    self.uptime.as_secs() / 3600,
                                                    (self.uptime.as_secs() % 3600) / 60,
                                                    self.uptime.as_secs() % 60
                                                ))
                                                .size(24)
                                            )
                                    )
                                    .padding(20)
                                    .style(iced::theme::Container::Box)
                                    .width(Length::Fixed(200.0))
                                )
                        )
                    )
                    .push(
                        container(
                            Row::new()
                                .spacing(20)
                                .push(
                                    // System Performance Section
                                    container(
                                        Column::new()
                                            .spacing(15)
                                            .push(text("System Performance").size(18))
                                            .push(
                                                Row::new()
                                                    .spacing(10)
                                                    .push(text("CPU Usage:").width(Length::Fixed(120.0)))
                                                    .push(
                                                        container(
                                                            text(format!("{:.1}%", cpu_usage))
                                                                .style(if cpu_usage > 80.0 {
                                                                    Color::from_rgb(0.8, 0.0, 0.0)
                                                                } else if cpu_usage > 60.0 {
                                                                    Color::from_rgb(0.8, 0.8, 0.0)
                                                                } else {
                                                                    Color::from_rgb(0.0, 0.8, 0.0)
                                                                })
                                                        )
                                                        .width(Length::Fill)
                                                    )
                                            )
                                            .push(
                                                Row::new()
                                                    .spacing(10)
                                                    .push(text("Memory Usage:").width(Length::Fixed(120.0)))
                                                    .push(
                                                        container(
                                                            text(format!("{:.1}% ({:.1}GB / {:.1}GB)", 
                                                                memory_usage,
                                                                used_memory / 1024.0,
                                                                total_memory / 1024.0
                                                            ))
                                                            .style(if memory_usage > 80.0 {
                                                                Color::from_rgb(0.8, 0.0, 0.0)
                                                            } else if memory_usage > 60.0 {
                                                                Color::from_rgb(0.8, 0.8, 0.0)
                                                            } else {
                                                                Color::from_rgb(0.0, 0.8, 0.0)
                                                            })
                                                        )
                                                        .width(Length::Fill)
                                                    )
                                            )
                                    )
                                    .padding(20)
                                    .style(iced::theme::Container::Box)
                                    .width(Length::Fill)
                                )
                        )
                    )
                    .push(
                        container(
                            Row::new()
                                .spacing(20)
                                .push(
                                    // Connection Metrics Section
                                    container(
                                        Column::new()
                                            .spacing(15)
                                            .push(text("Connection Metrics").size(18))
                                            .push(
                                                Row::new()
                                                    .spacing(10)
                                                    .push(text("Active:").width(Length::Fixed(120.0)))
                                                    .push(text(format!("{}", self.active_connections)))
                                            )
                                            .push(
                                                Row::new()
                                                    .spacing(10)
                                                    .push(text("Total:").width(Length::Fixed(120.0)))
                                                    .push(text(format!("{}", self.total_connections)))
                                            )
                                            .push(
                                                Row::new()
                                                    .spacing(10)
                                                    .push(text("Failed:").width(Length::Fixed(120.0)))
                                                    .push(
                                                        text(format!("{}", self.failed_connections))
                                                            .style(Color::from_rgb(0.8, 0.0, 0.0))
                                                    )
                                            )
                                    )
                                    .padding(20)
                                    .style(iced::theme::Container::Box)
                                    .width(Length::Fill)
                                )
                                .push(
                                    // LLM Metrics Section
                                    container(
                                        Column::new()
                                            .spacing(15)
                                            .push(text("LLM Metrics").size(18))
                                            .push(
                                                Row::new()
                                                    .spacing(10)
                                                    .push(text("Active Models:").width(Length::Fixed(120.0)))
                                                    .push(
                                                        text(format!("{}", 
                                                            self.llm_servers.iter()
                                                                .filter(|s| s.status == LLMStatus::Connected)
                                                                .count()
                                                        ))
                                                    )
                                            )
                                            .push(
                                                Row::new()
                                                    .spacing(10)
                                                    .push(text("Total Models:").width(Length::Fixed(120.0)))
                                                    .push(
                                                        text(format!("{}", 
                                                            self.llm_servers.iter()
                                                                .map(|s| s.available_models.len())
                                                                .sum::<usize>()
                                                        ))
                                                    )
                                            )
                                    )
                                    .padding(20)
                                    .style(iced::theme::Container::Box)
                                    .width(Length::Fill)
                                )
                        )
                    )
                    .push(
                        container(
                            Column::new()
                                .spacing(15)
                                .push(text("Recent Activity").size(18))
                                .push(self.view_log_section("System Logs", &self.server_logs))
                        )
                        .padding(20)
                        .style(iced::theme::Container::Box)
                        .width(Length::Fill)
                    )
                    .into()
            }
            View::Agents => self.view_agents(),
            View::Tasks => self.view_tasks(),
            View::Connections => self.view_connections(),
            View::Settings => self.view_settings(),
            View::LLMServers => self.view_llm_servers(),
        };

        let content = iced::widget::Row::new()
            .push(self.view_sidebar())
            .push(
                iced::widget::container(main_content)
                    .width(iced::Length::Fill)
                    .padding(20)
            );

        iced::widget::container(content)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }

    fn handle_agent_creation(&mut self) -> Command<Message> {
        let name = self.new_agent_name.clone();
        
        // Build MCP capabilities
        let mut capabilities = Vec::new();
        
        // Add data source capabilities
        if self.mcp_local_files {
            capabilities.push("mcp:data_source:local_files".to_string());
        }
        if self.mcp_databases {
            capabilities.push("mcp:data_source:databases".to_string());
        }
        if self.mcp_apis {
            capabilities.push("mcp:data_source:apis".to_string());
        }
        
        // Add tool capabilities
        if self.mcp_code_analysis {
            capabilities.push("mcp:tool:code_analysis".to_string());
        }
        if self.mcp_text_processing {
            capabilities.push("mcp:tool:text_processing".to_string());
        }
        if self.mcp_data_transform {
            capabilities.push("mcp:tool:data_transform".to_string());
        }
        
        // Add custom prompts if provided
        if !self.mcp_custom_prompts.is_empty() {
            if let Ok(prompts) = serde_json::from_str::<Vec<CustomPrompt>>(&self.mcp_custom_prompts) {
                for prompt in prompts {
                    capabilities.push(format!("mcp:prompts:{}:{}", prompt.name, prompt.template));
                }
            }
        }
        
        // Add root access if enabled
        if self.mcp_root_access {
            capabilities.push("mcp:root:enabled".to_string());
        }
        
        // Add transport configuration
        capabilities.push(format!("mcp:transport:{}", self.mcp_transport.to_lowercase()));

        let config = AgentConfig {
            max_concurrent_tasks: self.max_concurrent_tasks_input.parse().unwrap_or(5),
            priority_threshold: self.priority_threshold_input.parse().unwrap_or(2),
            llm_provider: self.new_llm_provider.clone(),
            llm_model: self.selected_model.clone(),
            retry_policy: RetryPolicy {
                max_retries: self.max_retries_input.parse().unwrap_or(3),
                backoff_ms: self.backoff_ms_input.parse().unwrap_or(1000),
                max_backoff_ms: self.max_backoff_ms_input.parse().unwrap_or(10000),
            },
            timeout_seconds: self.timeout_seconds_input.parse().unwrap_or(30),
        };

        let handler = self.handler.clone();
        Command::perform(async move {
            handler.deref().create_agent(name, config).await
        }, |result| Message::AgentCreated(result))
    }
}

// Custom style for the sidebar container
#[derive(Copy, Clone)]
struct SidebarStyle;

impl StyleSheet for SidebarStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(iced::Background::Color(iced::Color::from_rgb(0.0, 0.5, 0.7))),
            text_color: Some(iced::Color::WHITE),
            ..Default::default()
        }
    }
}

impl From<SidebarStyle> for iced::theme::Container {
    fn from(_: SidebarStyle) -> Self {
        iced::theme::Container::Custom(Box::new(SidebarStyle))
    }
}

impl Default for SidebarStyle {
    fn default() -> Self {
        SidebarStyle
    }
}

// Add custom style for error log entries
#[derive(Debug, Clone, Copy)]
struct ErrorLogStyle;

impl container::StyleSheet for ErrorLogStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(iced::Background::Color(Color::from_rgb(0.9, 0.8, 0.8))),
            border_radius: 4.0.into(),
            border_width: 0.0,
            ..Default::default()
        }
    }
}

impl From<ErrorLogStyle> for iced::theme::Container {
    fn from(_: ErrorLogStyle) -> Self {
        iced::theme::Container::Custom(Box::new(ErrorLogStyle))
    }
}

#[derive(Default)]
pub struct NexaGui {
    app: Option<NexaApp>,
}

impl Application for NexaGui {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = Arc<CliHandler>;

    fn new(flags: Self::Flags) -> (Self, Command<Message>) {
        let app = NexaApp::new(flags);
        
        (
            Self { app: Some(app) },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Nexa Core")
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        if let Some(app) = &mut self.app {
            match message {
                Message::Tick => {
                    if app.should_exit {
                        return window::close();
                    }
                    app.uptime += Duration::from_secs(1);
                    
                    // Check server status and update state
                    let handler = app.handler.clone();
                    if app.server_status == "Running" || app.server_status == "Starting" {
                        Command::perform(async move {
                            let server = handler.get_server();
                            let state = format!("{:?}", server.get_state().await);
                            let active = server.get_active_connections().await;
                            let metrics = server.get_metrics().await;
                            (state, active, metrics)
                        }, |(state, active, metrics)| Message::UpdateState(state, active, metrics))
                    } else {
                        Command::none()
                    }
                }
                Message::UpdateState(state, active, metrics) => {
                    // If we were in Starting state and got a valid state update, update the status
                    if app.server_status == "Starting" {
                        if state == "Running" {
                            app.server_status = "Running".to_string();
                            app.add_log("Server started successfully".to_string(), LogLevel::Info, "server");
                            app.add_log(format!("Active connections: {}", active), LogLevel::Debug, "server");
                        } else if state == "Error" || state == "Stopped" {
                            app.server_status = "Error".to_string();
                            app.add_log("Server failed to start".to_string(), LogLevel::Error, "error");
                        }
                    } else if active > 10 {
                        app.add_log(format!("High number of connections: {}", active), LogLevel::Warning, "connection");
                    }
                    app.server_status = state;
                    app.active_connections = active as u32;
                    app.total_connections = metrics.total_connections;
                    app.failed_connections = metrics.failed_connections;
                    if let Some(error) = metrics.last_error {
                        app.last_error = Some(error.clone());
                        app.add_log(error, LogLevel::Error, "error");
                    }
                    Command::none()
                }
                Message::StartServer => {
                    if app.server_status != "Starting" {  // Only start if not already starting
                        app.server_status = "Starting".to_string();
                        app.add_log("Starting server...".to_string(), LogLevel::Info, "server");
                        let handler = app.handler.clone();
                        Command::perform(async move {
                            match handler.start(None).await {
                                Ok(_) => (true, None),
                                Err(e) => (false, Some(e.to_string())),
                            }
                        }, |(success, error)| Message::ServerStarted(success, error))
                    } else {
                        Command::none()
                    }
                }
                Message::StopServer => {
                    if app.server_status != "Stopping" {  // Only stop if not already stopping
                        app.server_status = "Stopping".to_string();
                        app.add_log("Stopping server...".to_string(), LogLevel::Info, "server");
                        let handler = app.handler.clone();
                        Command::perform(async move {
                            match handler.stop().await {
                                Ok(_) => (true, None),
                                Err(e) => (false, Some(e.to_string())),
                            }
                        }, |(success, error)| Message::ServerStopped(success, error))
                    } else {
                        Command::none()
                    }
                }
                Message::ServerStarted(success, error) => {
                    if success {
                        app.server_status = "Running".to_string();
                        app.last_error = None;
                        app.add_log("Server started successfully".to_string(), LogLevel::Info, "server");
                    } else {
                        app.server_status = "Error".to_string();
                        if let Some(err) = error {
                            app.last_error = Some(format!("Failed to start server: {}", err));
                            app.add_log(format!("Failed to start server: {}", err), LogLevel::Error, "error");
                        }
                    }
                    Command::none()
                }
                Message::ServerStopped(success, error) => {
                    if success {
                        app.server_status = "Stopped".to_string();
                        app.last_error = None;
                        app.total_connections = 0;
                        app.active_connections = 0;
                        app.failed_connections = 0;
                        app.add_log("Server stopped successfully".to_string(), LogLevel::Info, "server");
                    } else {
                        app.server_status = "Error".to_string();
                        if let Some(err) = error {
                            app.last_error = Some(format!("Failed to stop server: {}", err));
                            app.add_log(format!("Failed to stop server: {}", err), LogLevel::Error, "error");
                        }
                    }
                    Command::none()
                }
                Message::Exit => {
                    app.should_exit = true;
                    Command::none()
                }
                Message::CreateAgent => {
                    app.handle_agent_creation()
                }
                Message::AgentNameChanged(name) => {
                    app.new_agent_name = name;
                    Command::none()
                }
                Message::AgentCapabilitiesChanged(capabilities) => {
                    app.new_agent_capabilities = capabilities;
                    Command::none()
                }
                Message::AgentCreated(result) => {
                    match result {
                        Ok(agent) => {
                            app.agents.push(agent.clone());
                            app.add_log(format!("Agent {} created successfully", agent.name), LogLevel::Info, "server");
                        },
                        Err(e) => {
                            app.add_log(format!("Failed to create agent: {}", e), LogLevel::Error, "error");
                        }
                    }
                    Command::none()
                }
                Message::CreateTask => {
                    let description = app.new_task_description.clone();
                    let priority = app.new_task_priority;
                    let agent_id = app.selected_agent.clone().unwrap_or_default();
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().create_task(description, priority, agent_id).await
                    }, Message::TaskCreated)
                }
                Message::TaskDescriptionChanged(description) => {
                    app.new_task_description = description;
                    Command::none()
                }
                Message::TaskPriorityChanged(priority) => {
                    app.new_task_priority = priority;
                    Command::none()
                }
                Message::TaskAssignedAgent(agent_id) => {
                    app.selected_agent = Some(agent_id);
                    Command::none()
                }
                Message::TaskCreated(result) => {
                    match result {
                        Ok(_) => {
                            let priority_val = match app.new_task_priority {
                                TaskPriority::Low => 0,
                                TaskPriority::Normal => 1,
                                TaskPriority::High => 2,
                                TaskPriority::Critical => 3,
                            };
                            app.tasks.push(Task {
                                id: Utc::now().timestamp_millis().to_string(),
                                title: app.new_task_description.clone(),
                                description: app.new_task_description.clone(),
                                status: TaskStatus::Pending,
                                steps: Vec::new(),
                                requirements: Vec::new(),
                                assigned_agent: app.selected_agent.clone(),
                                created_at: Utc::now(),
                                deadline: None,
                                priority: priority_val,
                                estimated_duration: 3600,
                            });
                            app.new_task_description.clear();
                            app.selected_agent = None;
                            app.add_log("Task created successfully".to_string(), LogLevel::Info, "server");
                        },
                        Err(e) => {
                            app.add_log(format!("Failed to create task: {}", e), LogLevel::Error, "error");
                        }
                    }
                    Command::none()
                }
                Message::SetMaxConnections(input) => {
                    app.max_connections_input = input.clone();
                    if let Ok(max) = input.parse::<u32>() {
                        let handler = app.handler.clone();
                        Command::perform(async move {
                            handler.deref().set_max_connections(max).await
                        }, Message::MaxConnectionsUpdated)
                    } else {
                        Command::none()
                    }
                }
                Message::MaxConnectionsUpdated(result) => {
                    match result {
                        Ok(_) => {
                            app.add_log("Max connections updated successfully".to_string(), LogLevel::Info, "server");
                        },
                        Err(e) => {
                            app.add_log(format!("Failed to update max connections: {}", e), LogLevel::Error, "error");
                        }
                    }
                    Command::none()
                }
                Message::ChangeView(view) => {
                    app.current_view = view;
                    Command::none()
                }
                Message::LLMConnected(agent_id, provider, result) => {
                    // Update LLM server status
                    if let Some(server) = app.llm_servers.iter_mut().find(|s| s.provider == provider) {
                        match &result {
                            Ok(_) => {
                                server.status = LLMStatus::Connected;
                                server.last_error = None;
                                app.add_log(
                                    format!("LLM connection successful for provider {}", provider),
                                    LogLevel::Info,
                                    "connection"
                                );
                                // Update selected provider when connection is successful
                                app.selected_provider = provider.clone();
                            },
                            Err(ref e) => {
                                server.status = LLMStatus::Error;
                                server.last_error = Some(e.clone());
                                app.add_log(
                                    format!("LLM connection failed for provider {}: {}", provider, e),
                                    LogLevel::Error,
                                    "error"
                                );
                            }
                        }
                    }

                    // Handle agent-specific connection if agent_id is provided
                    if !agent_id.is_empty() {
                        match &result {
                            Ok(_) => {
                                app.add_log(
                                    format!("LLM connection successful for agent {} with provider {}", agent_id, provider),
                                    LogLevel::Info,
                                    "server"
                                );
                            },
                            Err(ref e) => {
                                app.add_log(
                                    format!("LLM connection failed for agent {} with provider {}: {}", agent_id, provider, e),
                                    LogLevel::Error,
                                    "error"
                                );
                            }
                        }
                    }
                    Command::none()
                }
                Message::AddLLMServer(provider) => {
                    let address = app.new_llm_address.clone();
                    let provider_clone = provider.clone();
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().add_llm_server(&provider, &address).await
                    }, move |result| Message::LLMConnected(String::new(), provider_clone, result))
                }
                Message::RemoveLLMServer(provider) => {
                    let provider_clone = provider.clone();
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().remove_llm_server(&provider).await
                    }, move |result| Message::LLMConnected(String::new(), provider_clone, result))
                }
                Message::LLMAddressChanged(address) => {
                    app.new_llm_address = address;
                    Command::none()
                }
                Message::LLMProviderChanged(provider) => {
                    app.new_llm_provider = provider;
                    Command::none()
                }
                Message::ConnectLLM(provider) => {
                    // Set status to Connecting before initiating connection
                    if let Some(server) = app.llm_servers.iter_mut().find(|s| s.provider == provider) {
                        server.status = LLMStatus::Connecting;
                    }
                    let provider_clone = provider.clone();
                    let handler = app.handler.clone();
                    
                    Command::perform(async move {
                        match handler.deref().connect_llm(&provider).await {
                            Ok(_) => handler.deref().list_models(&provider).await,
                            Err(e) => Err(e)
                        }
                    }, move |result| Message::ModelsLoaded(provider_clone, result))
                }
                Message::DisconnectLLM(provider) => {
                    let provider_clone = provider.clone();
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().disconnect_llm(&provider).await
                    }, move |result| Message::LLMConnected(String::new(), provider_clone, result))
                }
                Message::ModelsLoaded(provider, result) => {
                    let (log_message, success) = if let Some(server) = app.llm_servers.iter_mut().find(|s| s.provider == provider) {
                        match &result {
                            Ok(models) => {
                                server.available_models = models.clone();
                                server.model_names = models.iter().map(|m| m.name.clone()).collect();
                                server.status = LLMStatus::Connected;
                                (format!("Loaded {} models for provider {}", models.len(), provider), true)
                            }
                            Err(e) => {
                                server.status = LLMStatus::Error;
                                server.last_error = Some(e.clone());
                                (format!("Failed to load models for provider {}: {}", provider, e), false)
                            }
                        }
                    } else {
                        (format!("Server not found: {}", provider), false)
                    };

                    app.add_log(
                        log_message,
                        if success { LogLevel::Info } else { LogLevel::Error },
                        if success { "connection" } else { "error" }
                    );
                    Command::none()
                }
                Message::SelectModel(provider, model) => {
                    if let Some(server) = app.llm_servers.iter_mut().find(|s| s.provider == provider) {
                        server.selected_model = Some(model.clone());
                        // Update selected model when selection is successful
                        app.selected_model = model.clone();
                    }
                    let handler = app.handler.clone();
                    let provider_clone = provider.clone();
                    let model_clone = model.clone();
                    Command::perform(async move {
                        handler.deref().select_model(&provider, &model).await
                    }, move |result| Message::ModelSelected(provider_clone, model_clone, result))
                }
                Message::ModelSelected(provider, model, result) => {
                    match result {
                        Ok(_) => {
                            app.add_log(
                                format!("Selected model {} for provider {}", model, provider),
                                LogLevel::Info,
                                "connection"
                            );
                        }
                        Err(e) => {
                            if let Some(server) = app.llm_servers.iter_mut().find(|s| s.provider == provider) {
                                server.last_error = Some(e.clone());
                            }
                            app.add_log(
                                format!("Failed to select model {} for provider {}: {}", model, provider, e),
                                LogLevel::Error,
                                "error"
                            );
                        }
                    }
                    Command::none()
                }
                Message::RefreshModels(provider) => {
                    let provider_clone = provider.clone();
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().list_models(&provider).await
                    }, move |result| Message::ModelsLoaded(provider_clone, result))
                }
                Message::TestModel(provider, model) => {
                    let provider_clone = provider.clone();
                    let model_clone = model.clone();
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().test_model(&provider, &model).await
                    }, move |result| Message::ModelTested(provider_clone, model_clone, result))
                }
                Message::ModelTested(provider, model, result) => {
                    match result {
                        Ok(response) => {
                            app.add_log(
                                format!("Model test successful for {} ({}): {}", model, provider, response),
                                LogLevel::Info,
                                "connection"
                            );
                        }
                        Err(e) => {
                            if let Some(server) = app.llm_servers.iter_mut().find(|s| s.provider == provider) {
                                server.last_error = Some(e.clone());
                            }
                            app.add_log(
                                format!("Model test failed for {} ({}): {}", model, provider, e),
                                LogLevel::Error,
                                "error"
                            );
                        }
                    }
                    Command::none()
                }
                Message::CreateNewAgent(name, config) => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().create_agent(name, config).await
                    }, Message::AgentCreated)
                }
                Message::CreateNewWorkflow(name, steps) => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().create_workflow(name, steps).await
                    }, Message::WorkflowCreated)
                }
                Message::WorkflowCreated(result) => {
                    match result {
                        Ok(workflow) => {
                            app.add_log(format!("Workflow {} created successfully", workflow.name), LogLevel::Info, "server");
                        },
                        Err(e) => {
                            app.add_log(format!("Failed to create workflow: {}", e), LogLevel::Error, "error");
                        }
                    }
                    Command::none()
                }
                Message::ExecuteWorkflow(workflow_id) => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().execute_workflow(&workflow_id).await
                    }, Message::WorkflowExecuted)
                }
                Message::WorkflowExecuted(result) => {
                    match result {
                        Ok(_) => {
                            app.add_log("Workflow executed successfully".to_string(), LogLevel::Info, "server");
                        },
                        Err(e) => {
                            app.add_log(format!("Workflow execution failed: {}", e), LogLevel::Error, "error");
                        }
                    }
                    Command::none()
                }
                Message::UpdateAgentCapabilities(agent_id, capabilities) => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().update_agent_capabilities(&agent_id, capabilities).await
                    }, Message::CapabilitiesUpdated)
                }
                Message::CapabilitiesUpdated(result) => {
                    match result {
                        Ok(_) => {
                            app.add_log("Agent capabilities updated successfully".to_string(), LogLevel::Info, "server");
                        },
                        Err(e) => {
                            app.add_log(format!("Failed to update capabilities: {}", e), LogLevel::Error, "error");
                        }
                    }
                    Command::none()
                }
                Message::SetAgentHierarchy(parent_id, child_id) => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().set_agent_hierarchy(&parent_id, &child_id).await
                    }, Message::HierarchyUpdated)
                }
                Message::HierarchyUpdated(result) => {
                    match result {
                        Ok(_) => {
                            app.add_log("Agent hierarchy updated successfully".to_string(), LogLevel::Info, "server");
                        },
                        Err(e) => {
                            app.add_log(format!("Failed to update hierarchy: {}", e), LogLevel::Error, "error");
                        }
                    }
                    Command::none()
                }
                Message::MaxConcurrentTasksChanged(value) => {
                    app.max_concurrent_tasks_input = value;
                    Command::none()
                }
                Message::PriorityThresholdChanged(value) => {
                    app.priority_threshold_input = value;
                    Command::none()
                }
                Message::MaxRetriesChanged(value) => {
                    app.max_retries_input = value;
                    Command::none()
                }
                Message::BackoffMsChanged(value) => {
                    app.backoff_ms_input = value;
                    Command::none()
                }
                Message::MaxBackoffMsChanged(value) => {
                    app.max_backoff_ms_input = value;
                    Command::none()
                }
                Message::TimeoutSecondsChanged(value) => {
                    app.timeout_seconds_input = value;
                    Command::none()
                }
                Message::TestAgent(id) => {
                    let handler = app.handler.clone();
                    Command::perform(
                        async move {
                            handler.test_agent(&id).await
                        },
                        |result| match result {
                            Ok(_) => Message::AgentTested,
                            Err(e) => Message::Error(e),
                        }
                    )
                }
                Message::ToggleAgentStatus(agent_id) => {
                    let handler = app.handler.clone();
                    Command::perform(
                        async move {
                            handler.deref().update_agent_capabilities(&agent_id, vec![]).await?;
                            Ok(())
                        },
                        |result: Result<(), String>| match result {
                            Ok(_) => Message::AgentStatusToggled,
                            Err(e) => Message::Error(e),
                        }
                    )
                }
                Message::DeleteAgent(agent_id) => {
                    let handler = app.handler.clone();
                    Command::perform(
                        async move {
                            handler.deref().update_agent_capabilities(&agent_id, vec![]).await?;
                            Ok(())
                        },
                        |result: Result<(), String>| match result {
                            Ok(_) => Message::AgentDeleted,
                            Err(e) => Message::Error(e),
                        }
                    )
                }
                Message::AgentTested => {
                    // Refresh agent list after testing
                    let handler = app.handler.clone();
                    Command::perform(
                        async move {
                            handler.list_agents(None).await
                        },
                        |result| match result {
                            Ok(agents) => Message::AgentsLoaded(agents),
                            Err(e) => Message::Error(e),
                        }
                    )
                }
                Message::AgentStatusToggled => {
                    // Refresh agent list after status toggle
                    let handler = app.handler.clone();
                    Command::perform(
                        async move {
                            handler.list_agents(None).await
                        },
                        |result| match result {
                            Ok(agents) => Message::AgentsLoaded(agents),
                            Err(e) => Message::Error(e),
                        }
                    )
                }
                Message::AgentDeleted => {
                    // Refresh agent list after deletion
                    let handler = app.handler.clone();
                    Command::perform(
                        async move {
                            handler.list_agents(None).await
                        },
                        |result| match result {
                            Ok(agents) => Message::AgentsLoaded(agents),
                            Err(e) => Message::Error(e),
                        }
                    )
                }
                Message::AgentsLoaded(agents) => {
                    app.agents = agents;
                    Command::none()
                }
                Message::Error(e) => {
                    app.add_log(format!("Error: {}", e), LogLevel::Error, "error");
                    Command::none()
                }
                Message::ToggleMCPLocalFiles(value) => {
                    app.mcp_local_files = value;
                    Command::none()
                }
                Message::ToggleMCPDatabases(value) => {
                    app.mcp_databases = value;
                    Command::none()
                }
                Message::ToggleMCPAPIs(value) => {
                    app.mcp_apis = value;
                    Command::none()
                }
                Message::ToggleMCPCodeAnalysis(value) => {
                    app.mcp_code_analysis = value;
                    Command::none()
                }
                Message::ToggleMCPTextProcessing(value) => {
                    app.mcp_text_processing = value;
                    Command::none()
                }
                Message::ToggleMCPDataTransform(value) => {
                    app.mcp_data_transform = value;
                    Command::none()
                }
                Message::MCPCustomPromptsChanged(prompts) => {
                    app.mcp_custom_prompts = prompts;
                    Command::none()
                }
                Message::ToggleMCPRootAccess(value) => {
                    app.mcp_root_access = value;
                    Command::none()
                }
                Message::MCPTransportChanged(transport) => {
                    app.mcp_transport = transport;
                    Command::none()
                }
            }
        } else {
            Command::none()
        }
    }

    fn view(&self) -> Element<Message> {
        if let Some(app) = &self.app {
            app.view()
        } else {
            text("Initializing...").into()
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

pub fn run_gui(handler: Arc<CliHandler>) -> Result<(), NexaError> {
    info!("Starting Nexa GUI...");
    let settings = Settings::with_flags(handler);

    // Run the GUI on the main thread
    tokio::task::block_in_place(|| {
        NexaGui::run(settings)
            .map_err(|e| NexaError::System(format!("Failed to run GUI: {}", e)))
    })
} 