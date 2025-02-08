use iced::{
    keyboard,
    widget::{
        button, checkbox, column, container, horizontal_rule, horizontal_space, pick_list, row, text,
        text_input, vertical_space, Text, Row,
    },
    Task,
    Element, Length, Size, Subscription, window,
};
use log::{debug, error, info};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use std::path::PathBuf;

use crate::{
    server::{Server, ServerState, ServerMetrics},
    models::agent::{Agent, Task as AgentTask},
    cli::{AgentConfig, LLMModel},
    gui::components::{
        dashboard::{self, DashboardMetrics},
        agents::{self, AgentConfigState},
        workflows::{self},
        tasks::{self},
        settings::{self},
        logs::{self},
        styles,
    },
    settings::{SettingsManager, LLMServerConfig},
    logging,
    error::NexaError,
    cli::CliHandler,
};

// Constants for UI configuration
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_WINDOW_SIZE: Size = Size::new(1024.0, 768.0);
const MAX_LOGS: usize = 1000;

pub fn main() -> iced::Result {
    let example = Example::new().0;
    iced::application(example.title(), Example::update, Example::view)
        .subscription(Example::subscription)
        .window_size(DEFAULT_WINDOW_SIZE)
        .theme(|_| iced::Theme::Dark)
        .run()
}

/// Navigation views
#[derive(Debug, Clone)]
pub enum View {
    Dashboard,
    Agents,
    Settings,
    Logs,
    Workflows,
    Tasks,
}

/// LLM settings state
#[derive(Debug, Clone)]
pub struct LLMSettingsState {
    pub servers: Vec<LLMServerConfig>,
    pub available_models: Vec<LLMModel>,
    pub new_server_url: String,
    pub new_server_provider: String,
}

impl Default for LLMSettingsState {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            available_models: Vec::new(),
            new_server_url: String::new(),
            new_server_provider: String::new(),
        }
    }
}

/// Dock item for navigation
#[derive(Debug, Clone)]
pub struct DockItem {
    pub name: String,
    pub icon: String,
    pub action: Message,
    pub shortcut: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskState {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub assigned_agent: Option<String>,
    pub priority: TaskPriority,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Main application state
pub struct Example {
    // Core state
    #[allow(dead_code)]
    cli_handler: std::sync::Arc<crate::cli::CliHandler>,
    current_view: View,
    settings_manager: SettingsManager,
    
    // Component states
    agents: Vec<Agent>,
    selected_agent: Option<String>,
    logs: Vec<String>,
    config_state: AgentConfigState,
    llm_settings: LLMSettingsState,
    dock_items: Vec<DockItem>,
    tasks: Vec<AgentTask>,
    server_metrics: ServerMetrics,
    
    // Channels
    _log_receiver: mpsc::UnboundedReceiver<String>,
    server: Server,
    last_metrics_update: Instant,
    state: ServerState,
}

/// Application messages
#[derive(Debug, Clone)]
pub enum Message {
    // Component Messages
    AgentMessage(agents::AgentMessage),
    WorkflowMessage(workflows::WorkflowMessage),
    TaskMessage(tasks::TaskMessage),
    SettingsMessage(settings::SettingsMessage),
    LogMessage(logs::LogMessage),
    
    // View Management
    ChangeView(View),
    Tick,
    
    // System
    Batch(Vec<Message>),
    UpdateServerMetrics(ServerMetrics),
    UpdateAgents(Vec<Agent>),
    UpdateModels(Vec<LLMModel>),
    
    // Server Control
    StartServer,
    StopServer,
    ServerStateChanged(ServerState),
    
    // Logging
    LogReceived(String),
    ServerStarted(Result<(), NexaError>),
    ServerStopped(Result<(), NexaError>),
    MetricsUpdated(ServerMetrics),
}

// New helper types for better code organization
#[derive(Debug, Clone)]
pub enum LLMAction {
    Connect { provider: String, url: String },
    Disconnect(String),
}

#[derive(Debug, Clone)]
pub enum ConfigUpdate {
    MaxTasks(u32),
    Priority(u32),
    Provider(String),
    Model(String),
    Timeout(u32),
}

#[derive(Debug, Clone)]
pub enum AgentControlAction {
    Start(String),
    Stop(String),
    Update(String, AgentConfig),
}

impl Example {
    fn new() -> (Self, Task<Message>) {
        let settings_manager = SettingsManager::new();
        let settings = settings_manager.get().clone();

        // Set up logging channel
        let (log_sender, log_receiver) = mpsc::unbounded_channel();
        logging::set_ui_sender(log_sender.clone());
        
        // Send initial log
        let _ = log_sender.send("Nexa UI started. Initializing components...".to_string());

        let dock_items = vec![
            DockItem {
                name: "Dashboard".to_string(),
                icon: "📊".to_string(),
                action: Message::ChangeView(View::Dashboard),
                shortcut: Some("Ctrl+D".to_string()),
            },
            DockItem {
                name: "Agents".to_string(),
                icon: "👥".to_string(),
                action: Message::ChangeView(View::Agents),
                shortcut: Some("Ctrl+A".to_string()),
            },
            DockItem {
                name: "Settings".to_string(),
                icon: "⚙️".to_string(),
                action: Message::ChangeView(View::Settings),
                shortcut: Some("Ctrl+S".to_string()),
            },
            DockItem {
                name: "Logs".to_string(),
                icon: "📝".to_string(),
                action: Message::ChangeView(View::Logs),
                shortcut: Some("Ctrl+L".to_string()),
            },
            DockItem {
                name: "Workflows".to_string(),
                icon: "🔄".to_string(),
                action: Message::ChangeView(View::Workflows),
                shortcut: Some("Ctrl+W".to_string()),
            },
            DockItem {
                name: "Tasks".to_string(),
                icon: "✅".to_string(),
                action: Message::ChangeView(View::Tasks),
                shortcut: Some("Ctrl+T".to_string()),
            },
        ];

        let server = Server::new(
            PathBuf::from("nexa.pid"),
            PathBuf::from("nexa.sock"),
        );

        (
            Example {
                cli_handler: std::sync::Arc::new(crate::cli::CliHandler::new()),
                current_view: View::Dashboard,
                settings_manager,
                agents: Vec::new(),
                selected_agent: None,
                logs: Vec::new(),
                config_state: AgentConfigState::default(),
                llm_settings: LLMSettingsState {
                    servers: settings.llm_servers,
                    available_models: Vec::new(),
                    new_server_url: String::new(),
                    new_server_provider: String::new(),
                },
                dock_items,
                tasks: Vec::new(),
                server_metrics: ServerMetrics::new(),
                _log_receiver: log_receiver,
                server,
                last_metrics_update: Instant::now(),
                state: ServerState::Stopped,
            },
            Task::none(),
        )
    }

    fn title(&self) -> &'static str {
        "Nexa Agent Management"
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        debug!("Handling message: {:?}", message);
        
        match message {
            Message::AgentMessage(msg) => {
                match msg {
                    agents::AgentMessage::ViewDetails(id) => {
                        self.selected_agent = Some(id);
                        Task::none()
                    }
                    agents::AgentMessage::Back => {
                        self.selected_agent = None;
                        Task::none()
                    }
                    agents::AgentMessage::Start(id) => {
                        if let Some(_agent) = self.agents.iter_mut().find(|a| a.id == id) {
                            // Update agent status
                            debug!("Starting agent {}", id);
                        }
                        Task::none()
                    }
                    agents::AgentMessage::Stop(id) => {
                        if let Some(_agent) = self.agents.iter_mut().find(|a| a.id == id) {
                            // Update agent status
                            debug!("Stopping agent {}", id);
                        }
                        Task::none()
                    }
                    _ => Task::none()
                }
            }
            Message::WorkflowMessage(msg) => {
                match msg {
                    workflows::WorkflowMessage::ViewDetails(id) => {
                        debug!("Viewing workflow details {}", id);
                        Task::none()
                    }
                    workflows::WorkflowMessage::Execute(id) => {
                        debug!("Executing workflow {}", id);
                        Task::none()
                    }
                    _ => Task::none()
                }
            }
            Message::TaskMessage(msg) => {
                match msg {
                    tasks::TaskMessage::ViewDetails(id) => {
                        debug!("Viewing task details {}", id);
                        Task::none()
                    }
                    tasks::TaskMessage::UpdateStatus(id, status) => {
                        debug!("Updating task {} status to {:?}", id, status);
                        Task::none()
                    }
                    _ => Task::none()
                }
            }
            Message::SettingsMessage(msg) => {
                match msg {
                    settings::SettingsMessage::AddServer(url, provider) => {
                        let server_config = LLMServerConfig {
                            provider: provider.clone(),
                            url: url.clone(),
                            models: Vec::new(),
                        };
                        self.llm_settings.servers.push(server_config.clone());
                        
                        // Update persistent settings
                        let _ = self.settings_manager.update(|settings| {
                            settings.llm_servers.push(server_config);
                        });

                        self.llm_settings.new_server_url.clear();
                        self.llm_settings.new_server_provider.clear();
                        Task::none()
                    }
                    settings::SettingsMessage::RemoveServer(provider) => {
                        self.llm_settings.servers.retain(|s| s.provider != provider);
                        
                        // Update persistent settings
                        let _ = self.settings_manager.update(|settings| {
                            settings.llm_servers.retain(|s| s.provider != provider);
                        });
                        Task::none()
                    }
                    settings::SettingsMessage::UpdateNewServerUrl(url) => {
                        self.llm_settings.new_server_url = url;
                        Task::none()
                    }
                    settings::SettingsMessage::UpdateNewServerProvider(provider) => {
                        self.llm_settings.new_server_provider = provider;
                        Task::none()
                    }
                    _ => Task::none()
                }
            }
            Message::LogMessage(msg) => {
                match msg {
                    logs::LogMessage::Clear => {
                        debug!("Clearing logs from UI");
                        self.logs.clear();
                        Task::none()
                    }
                    logs::LogMessage::Show(log) => {
                        debug!("Adding log to UI: {}", log);
                        self.logs.push(log);
                        if self.logs.len() > MAX_LOGS {
                            self.logs.remove(0);
                        }
                        Task::none()
                    }
                }
            }
            Message::LogReceived(log) => {
                self.logs.push(log);
                if self.logs.len() > self.settings_manager.get().max_logs {
                    self.logs.remove(0);
                }
                Task::none()
            }
            Message::ChangeView(view) => {
                debug!("Changing view to: {:?}", view);
                self.current_view = view;
                Task::none()
            }
            Message::Tick => {
                if self.state == ServerState::Running && 
                   self.last_metrics_update.elapsed() >= Duration::from_secs(1) {
                    let server = self.server.clone();
                    self.last_metrics_update = Instant::now();
                    Task::perform(async move {
                        server.get_metrics().await
                    }, Message::MetricsUpdated)
                } else {
                    Task::none()
                }
            }
            Message::UpdateServerMetrics(metrics) => {
                self.server_metrics = metrics;
                Task::none()
            }
            Message::UpdateAgents(agents) => {
                self.agents = agents;
                Task::none()
            }
            Message::UpdateModels(models) => {
                self.llm_settings.available_models = models;
                Task::none()
            }
            Message::Batch(messages) => {
                for message in messages {
                    if let Message::UpdateServerMetrics(metrics) = message {
                        self.server_metrics = metrics;
                    } else if let Message::UpdateAgents(agents) = message {
                        self.agents = agents;
                    } else if let Message::UpdateModels(models) = message {
                        self.llm_settings.available_models = models;
                    }
                }
                Task::none()
            }
            Message::StartServer => {
                self.logs.push("Starting server...".to_string());
                let server = self.server.clone();
                Task::perform(async move {
                    match server.start().await {
                        Ok(_) => {
                            debug!("Server started successfully");
                            Ok(())
                        }
                        Err(e) => {
                            error!("Failed to start server: {}", e);
                            Err(e)
                        }
                    }
                }, Message::ServerStarted)
            }
            Message::StopServer => {
                self.logs.push("Stopping server...".to_string());
                let server = self.server.clone();
                Task::perform(async move {
                    match server.stop().await {
                        Ok(_) => {
                            debug!("Server stopped successfully");
                            Ok(())
                        }
                        Err(e) => {
                            error!("Failed to stop server: {}", e);
                            Err(e)
                        }
                    }
                }, Message::ServerStopped)
            }
            Message::ServerStarted(result) => {
                match result {
                    Ok(_) => {
                        self.logs.push("Server started successfully".to_string());
                        self.state = ServerState::Running;
                    }
                    Err(e) => {
                        self.logs.push(format!("Failed to start server: {}", e));
                        self.state = ServerState::Stopped;
                    }
                }
                Task::none()
            }
            Message::ServerStopped(result) => {
                match result {
                    Ok(_) => {
                        self.logs.push("Server stopped successfully".to_string());
                        self.state = ServerState::Stopped;
                        self.server_metrics = ServerMetrics::default();
                    }
                    Err(e) => {
                        self.logs.push(format!("Failed to stop server: {}", e));
                    }
                }
                Task::none()
            }
            Message::MetricsUpdated(metrics) => {
                self.server_metrics = metrics;
                Task::none()
            }
            Message::ServerStateChanged(state) => {
                debug!("Server state changed to: {:?}", state);
                self.state = state;
                Task::none()
            }
            _ => Task::none()
        }
    }

    fn view(&self) -> Element<Message> {
        let dock = self.view_dock();
        let content = match self.current_view {
            View::Dashboard => {
                let metrics = self.get_metrics();
                dashboard::view_dashboard(metrics)
            }
            View::Agents => {
                if let Some(agent_id) = &self.selected_agent {
                    if let Some(agent) = self.agents.iter().find(|a| &a.id == agent_id) {
                        agents::view_agent_details(agent, &self.config_state)
                            .map(Message::AgentMessage)
                    } else {
                        container(
                            Text::new("Agent not found")
                                .size(32)
                                .style(styles::header_text)
                        )
                        .padding(20)
                        .style(styles::panel_content)
                        .into()
                    }
                } else {
                    agents::view_agents_list(&self.agents)
                        .map(Message::AgentMessage)
                }
            }
            View::Settings => {
                let header = settings::view_settings_header()
                    .map(|msg| Message::SettingsMessage(msg));
                let add_server_form = settings::view_add_server_form(
                    &self.llm_settings.new_server_url,
                    &self.llm_settings.new_server_provider,
                )
                .map(|msg| Message::SettingsMessage(msg));
                let servers_list = settings::view_servers_list(
                    &self.llm_settings.servers,
                    &self.llm_settings.available_models,
                )
                .map(|msg| Message::SettingsMessage(msg));

                container(
                    column![
                        header,
                        add_server_form,
                        servers_list
                    ]
                    .spacing(20)
                )
                .padding(20)
                .style(styles::panel_content)
                .into()
            }
            View::Logs => {
                logs::view_logs_panel(&self.logs)
            }
            View::Workflows => {
                workflows::view_workflow_header()
                    .map(Message::WorkflowMessage)
            }
            View::Tasks => {
                tasks::view_task_header()
                    .map(Message::TaskMessage)
            }
        };

        container(
            column![
                dock,
                content,
            ]
            .spacing(20)
        )
        .padding(20)
        .style(styles::main_container)
        .into()
    }

    fn view_dock(&self) -> Element<Message> {
        let dock_items = self.dock_items.iter().map(|item| {
            let action = item.action.clone();
            button(
                container(
                    column![
                        Text::new(&item.icon).size(24),
                        Text::new(&item.name).size(12)
                    ]
                    .spacing(5)
                    .width(Length::Fill)
                    .align_x(iced::Alignment::Center)
                )
                .padding(10)
                .style(styles::dock_item)
            )
            .on_press(action)
            .width(Length::Fill)
            .into()
        }).collect::<Vec<_>>();

        container(
            Row::with_children(dock_items)
                .spacing(10)
                .padding(10)
                .width(Length::Fill)
                .align_y(iced::Alignment::Center)
        )
        .width(Length::Fill)
        .style(styles::dock)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            keyboard::on_key_press(|key: keyboard::Key, _modifiers: keyboard::Modifiers| {
                match key {
                    keyboard::Key::Character(c) => match c.as_str() {
                        "d" => Some(Message::ChangeView(View::Dashboard)),
                        "a" => Some(Message::ChangeView(View::Agents)),
                        "s" => Some(Message::ChangeView(View::Settings)),
                        "l" => Some(Message::ChangeView(View::Logs)),
                        "w" => Some(Message::ChangeView(View::Workflows)),
                        "t" => Some(Message::ChangeView(View::Tasks)),
                        _ => None,
                    },
                    _ => None,
                }
            }),
            // Regular tick for UI updates
            iced::time::every(REFRESH_INTERVAL)
                .map(|_| Message::Tick),
        ])
    }

    fn get_metrics(&self) -> DashboardMetrics {
        DashboardMetrics::from_state(
            &self.agents,
            &self.server_metrics,
            &self.llm_settings.servers,
            &self.llm_settings.available_models,
            &self.tasks
        )
    }
}

impl Default for Example {
    fn default() -> Self {
        Example::new().0
    }
}