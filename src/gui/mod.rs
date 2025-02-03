use iced::widget::{container, row, text, Column, Button, scrollable, Rule, TextInput, PickList};
use iced::{Element, Length, Theme, Color, Subscription, Command};
use iced::executor;
use iced::window;
use iced::{Application, Settings};
use std::sync::Arc;
use std::time::Duration;
use std::collections::VecDeque;
use crate::server::ServerMetrics;
use crate::error::NexaError;
use crate::cli::CliHandler;
use crate::{Agent, Task, AgentStatus, TaskStatus};
use log::info;
use chrono::Utc;
use std::ops::Deref;

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
    AgentCreated(Result<(), String>),
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
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum View {
    Overview,
    Agents,
    Tasks,
    Connections,
    Settings,
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
}

#[derive(Debug, Clone, Copy)]
enum LogLevel {
    Info,
    Error,
    Debug,
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
    agents: Vec<Agent>,
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
        }
    }

    fn add_log(&mut self, message: String, level: LogLevel, log_type: &str) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            message,
            level,
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
        let mut log_content = Column::new()
            .spacing(5)
            .push(text(title).size(20));

        for entry in logs.iter() {
            let color = match entry.level {
                LogLevel::Info => Color::from_rgb(0.0, 0.8, 0.0),
                LogLevel::Error => Color::from_rgb(0.8, 0.0, 0.0),
                LogLevel::Debug => Color::from_rgb(0.5, 0.5, 0.5),
            };

            log_content = log_content.push(
                text(format!("[{}] {}", 
                    entry.timestamp.format("%H:%M:%S"),
                    entry.message
                )).style(color).size(14)
            );
        }

        scrollable(
            container(log_content)
                .width(Length::Fill)
                .padding(10)
        )
        .height(Length::Fixed(200.0))
        .into()
    }

    fn view_navigation(&self) -> Element<Message> {
        let nav_button = |label: &str, view: View| {
            let style = if self.current_view == view {
                iced::theme::Button::Primary
            } else {
                iced::theme::Button::Secondary
            };

            Button::new(text(label))
                .on_press(Message::ChangeView(view))
                .style(style)
        };

        row![
            nav_button("Overview", View::Overview),
            nav_button("Agents", View::Agents),
            nav_button("Tasks", View::Tasks),
            nav_button("Connections", View::Connections),
            nav_button("Settings", View::Settings),
        ]
        .spacing(10)
        .padding(10)
        .into()
    }

    fn view_agents(&self) -> Element<Message> {
        let mut content = Column::new()
            .spacing(20)
            .push(text("Agent Management").size(24))
            .push(
                Column::new()
                    .spacing(10)
                    .push(text("Create New Agent"))
                    .push(
                        TextInput::new("Enter agent name...", &self.new_agent_name)
                            .on_input(Message::AgentNameChanged)
                    )
                    .push(
                        TextInput::new("Enter capabilities (comma-separated)...", &self.new_agent_capabilities)
                            .on_input(Message::AgentCapabilitiesChanged)
                    )
                    .push(
                        Button::new("Create Agent")
                            .on_press(Message::CreateAgent)
                    )
            );

        // List existing agents
        content = content.push(Rule::horizontal(10))
            .push(text("Existing Agents").size(20));

        for agent in &self.agents {
            content = content.push(
                container(
                    Column::new()
                        .spacing(5)
                        .push(text(&format!("Name: {}", agent.id)))
                        .push(text(&format!("Status: {:?}", agent.status)))
                        .push(text(&format!("Capabilities: {}", agent.capabilities.join(", "))))
                )
                .padding(10)
                .style(iced::theme::Container::Box)
            );
        }

        scrollable(content).height(Length::Fill).into()
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

    pub fn view(&self) -> Element<Message> {
        let content = Column::new()
            .spacing(20)
            .padding(20)
            .push(self.view_navigation())
            .push(
                match self.current_view {
                    View::Overview => {
                        let status_color = match self.server_status.as_str() {
                            "Running" => Color::from_rgb(0.0, 0.8, 0.0),
                            "Stopped" => Color::from_rgb(0.8, 0.0, 0.0),
                            _ => Color::from_rgb(0.8, 0.8, 0.0),
                        };

                        Column::new()
                            .spacing(20)
                            .push(
                                text(&format!("Status: {}", self.server_status))
                                    .style(status_color)
                                    .size(24)
                            )
                            .push(
                                container(Column::new()
                                    .spacing(5)
                                    .push(text(format!("Uptime: {:?}", self.uptime)))
                                    .push(text(format!("Total Connections: {}", self.total_connections)))
                                    .push(text(format!("Active Connections: {}", self.active_connections)))
                                    .push(text(format!("Failed Connections: {}", self.failed_connections)))
                                )
                                .padding(10)
                            )
                            .push(Rule::horizontal(10))
                            .push(
                                if let Some(error) = &self.last_error {
                                    container(
                                        text(format!("Error: {}", error))
                                            .style(Color::from_rgb(0.8, 0.0, 0.0))
                                    )
                                    .padding(10)
                                } else {
                                    container(text("No errors"))
                                        .padding(10)
                                }
                            )
                            .push(Rule::horizontal(10))
                            .push(
                                row![
                                    Column::new()
                                        .width(Length::FillPortion(1))
                                        .push(self.view_log_section("Server Logs", &self.server_logs)),
                                    Column::new()
                                        .width(Length::FillPortion(1))
                                        .push(self.view_log_section("Connection Logs", &self.connection_logs)),
                                    Column::new()
                                        .width(Length::FillPortion(1))
                                        .push(self.view_log_section("Error Logs", &self.error_logs)),
                                ].spacing(20)
                            )
                            .into()
                    }
                    View::Agents => self.view_agents(),
                    View::Tasks => self.view_tasks(),
                    View::Connections => self.view_connections(),
                    View::Settings => self.view_settings(),
                }
            );

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
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
                    if app.server_status == "Running" {
                        let handler = app.handler.clone();
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
                    app.server_status = state;
                    app.active_connections = active as u32;
                    app.total_connections = metrics.total_connections;
                    app.failed_connections = metrics.failed_connections;
                    if let Some(error) = metrics.last_error {
                        app.last_error = Some(error.clone());
                        app.add_log(error, LogLevel::Error, "error");
                    }
                    app.add_log(
                        format!("Active: {}, Total: {}, Failed: {}", 
                            active, 
                            metrics.total_connections,
                            metrics.failed_connections
                        ),
                        LogLevel::Debug,
                        "connection"
                    );
                    Command::none()
                }
                Message::StartServer => {
                    app.add_log("Starting server...".to_string(), LogLevel::Info, "server");
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        match handler.start(None).await {
                            Ok(_) => (true, None),
                            Err(e) => (false, Some(e.to_string())),
                        }
                    }, |(success, error)| Message::ServerStarted(success, error))
                }
                Message::StopServer => {
                    app.add_log("Stopping server...".to_string(), LogLevel::Info, "server");
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        match handler.stop().await {
                            Ok(_) => (true, None),
                            Err(e) => (false, Some(e.to_string())),
                        }
                    }, |(success, error)| Message::ServerStopped(success, error))
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
                    let name = app.new_agent_name.clone();
                    let capabilities: Vec<String> = app.new_agent_capabilities
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.deref().create_agent(name, capabilities).await
                    }, Message::AgentCreated)
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
                        Ok(_) => {
                            app.agents.push(Agent {
                                id: app.new_agent_name.clone(),
                                name: app.new_agent_name.clone(),
                                capabilities: app.new_agent_capabilities
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect(),
                                status: AgentStatus::Idle,
                                current_task: None,
                                last_heartbeat: Utc::now(),
                            });
                            app.agent_options.push(app.new_agent_name.clone());
                            app.new_agent_name.clear();
                            app.new_agent_capabilities.clear();
                            app.add_log("Agent created successfully".to_string(), LogLevel::Info, "server");
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