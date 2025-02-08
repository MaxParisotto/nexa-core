use iced::keyboard;
use log::debug;
use iced::widget::{
    column, container, Text, Row, button,
};
use iced::{Element, Length, Size, Subscription, Task as IcedTask};
use crate::models::agent::{Agent, Task as AgentTask};
use crate::cli::{AgentConfig, LLMModel};
use crate::gui::components::dashboard::DashboardMetrics;
use crate::settings::{SettingsManager, LLMServerConfig};
use crate::server::{ServerMetrics, ServerState};
use crate::gui::components::{
    agents, workflows, tasks, settings, logs, styles, dashboard,
};
use crate::gui::components::agents::AgentConfigState;
use std::time::Duration;
use tokio::sync::mpsc;
use crate::logging;

// Constants for UI configuration
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_WINDOW_SIZE: Size = Size::new(1920.0, 1080.0);
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
    fn new() -> (Self, IcedTask<Message>) {
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
            },
            IcedTask::none(),
        )
    }

    fn title(&self) -> &'static str {
        "Nexa Agent Management"
    }

    fn update(&mut self, message: Message) -> IcedTask<Message> {
        debug!("Handling message: {:?}", message);
        
        match message {
            Message::AgentMessage(msg) => {
                match msg {
                    agents::AgentMessage::ViewDetails(id) => {
                        self.selected_agent = Some(id);
                        IcedTask::none()
                    }
                    agents::AgentMessage::Back => {
                        self.selected_agent = None;
                        IcedTask::none()
                    }
                    agents::AgentMessage::Start(id) => {
                        if let Some(_agent) = self.agents.iter_mut().find(|a| a.id == id) {
                            // Update agent status
                            debug!("Starting agent {}", id);
                        }
                        IcedTask::none()
                    }
                    agents::AgentMessage::Stop(id) => {
                        if let Some(_agent) = self.agents.iter_mut().find(|a| a.id == id) {
                            // Update agent status
                            debug!("Stopping agent {}", id);
                        }
                        IcedTask::none()
                    }
                    _ => IcedTask::none()
                }
            }
            Message::WorkflowMessage(msg) => {
                match msg {
                    workflows::WorkflowMessage::ViewDetails(id) => {
                        debug!("Viewing workflow details {}", id);
                        IcedTask::none()
                    }
                    workflows::WorkflowMessage::Execute(id) => {
                        debug!("Executing workflow {}", id);
                        IcedTask::none()
                    }
                    _ => IcedTask::none()
                }
            }
            Message::TaskMessage(msg) => {
                match msg {
                    tasks::TaskMessage::ViewDetails(id) => {
                        debug!("Viewing task details {}", id);
                        IcedTask::none()
                    }
                    tasks::TaskMessage::UpdateStatus(id, status) => {
                        debug!("Updating task {} status to {:?}", id, status);
                        IcedTask::none()
                    }
                    _ => IcedTask::none()
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
                        IcedTask::none()
                    }
                    settings::SettingsMessage::RemoveServer(provider) => {
                        self.llm_settings.servers.retain(|s| s.provider != provider);
                        
                        // Update persistent settings
                        let _ = self.settings_manager.update(|settings| {
                            settings.llm_servers.retain(|s| s.provider != provider);
                        });
                        IcedTask::none()
                    }
                    settings::SettingsMessage::UpdateNewServerUrl(url) => {
                        self.llm_settings.new_server_url = url;
                        IcedTask::none()
                    }
                    settings::SettingsMessage::UpdateNewServerProvider(provider) => {
                        self.llm_settings.new_server_provider = provider;
                        IcedTask::none()
                    }
                    _ => IcedTask::none()
                }
            }
            Message::LogMessage(msg) => {
                match msg {
                    logs::LogMessage::Clear => {
                        debug!("Clearing logs from UI");
                        self.logs.clear();
                        IcedTask::none()
                    }
                    logs::LogMessage::Show(log) => {
                        debug!("Adding log to UI: {}", log);
                        self.logs.push(log);
                        if self.logs.len() > MAX_LOGS {
                            self.logs.remove(0);
                        }
                        IcedTask::none()
                    }
                }
            }
            Message::LogReceived(log) => {
                self.logs.push(log);
                if self.logs.len() > self.settings_manager.get().max_logs {
                    self.logs.remove(0);
                }
                IcedTask::none()
            }
            Message::ChangeView(view) => {
                debug!("Changing view to: {:?}", view);
                self.current_view = view;
                IcedTask::none()
            }
            Message::Tick => {
                // Try to receive any pending log messages
                while let Ok(log) = self._log_receiver.try_recv() {
                    self.logs.push(log);
                    if self.logs.len() > self.settings_manager.get().max_logs {
                        self.logs.remove(0);
                    }
                }
                
                // Update metrics periodically
                let cli = self.cli_handler.clone();
                IcedTask::perform(
                    async move {
                        let metrics = cli.get_server().get_metrics().await;
                        Ok::<ServerMetrics, String>(metrics)
                    },
                    |result| match result {
                        Ok(metrics) => Message::UpdateServerMetrics(metrics),
                        Err(e) => {
                            debug!("Failed to update metrics: {}", e);
                            Message::LogReceived(format!("Error updating metrics: {}", e))
                        }
                    }
                )
            }
            Message::UpdateServerMetrics(metrics) => {
                self.server_metrics = metrics;
                IcedTask::none()
            }
            Message::UpdateAgents(agents) => {
                self.agents = agents;
                IcedTask::none()
            }
            Message::UpdateModels(models) => {
                self.llm_settings.available_models = models;
                IcedTask::none()
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
                IcedTask::none()
            }
            Message::StartServer => {
                debug!("Starting server...");
                let cli = self.cli_handler.clone();
                IcedTask::perform(
                    async move {
                        match cli.start(None).await {
                            Ok(_) => Ok(ServerState::Running),
                            Err(e) => Err(e.to_string())
                        }
                    },
                    |result| match result {
                        Ok(state) => {
                            debug!("Server started successfully");
                            Message::ServerStateChanged(state)
                        },
                        Err(e) => {
                            debug!("Failed to start server: {}", e);
                            Message::LogReceived(format!("Failed to start server: {}", e))
                        }
                    }
                )
            }
            Message::StopServer => {
                debug!("Stopping server...");
                let cli = self.cli_handler.clone();
                IcedTask::perform(
                    async move {
                        match cli.stop().await {
                            Ok(_) => Ok(ServerState::Stopped),
                            Err(e) => Err(e.to_string())
                        }
                    },
                    |result| match result {
                        Ok(state) => {
                            debug!("Server stopped successfully");
                            Message::ServerStateChanged(state)
                        },
                        Err(e) => {
                            debug!("Failed to stop server: {}", e);
                            Message::LogReceived(format!("Failed to stop server: {}", e))
                        }
                    }
                )
            }
            Message::ServerStateChanged(state) => {
                debug!("Server state changed to: {:?}", state);
                let cli = self.cli_handler.clone();
                IcedTask::perform(
                    async move {
                        let metrics = cli.get_server().get_metrics().await;
                        Ok::<ServerMetrics, String>(metrics)
                    },
                    |result| match result {
                        Ok(metrics) => Message::UpdateServerMetrics(metrics),
                        Err(e) => Message::LogReceived(format!("Error updating metrics: {}", e))
                    }
                )
            }
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