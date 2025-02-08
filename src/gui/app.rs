use iced::keyboard;
use log::debug;
use iced::widget::{
    column, container, Text, Row, button,
};
use iced::{Element, Length, Size, Subscription, Task};
use crate::models::agent::Agent;
use crate::cli::{AgentConfig, LLMModel};
use std::time::Duration;

use crate::gui::components::{
    agents, workflows, tasks, settings, logs, styles,
};
use crate::gui::components::agents::AgentConfigState;
use crate::gui::components::settings::LLMServerConfig;

// Constants for UI configuration
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
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
    Agents,
    Settings,
    Logs,
    Workflows,
    Tasks,
}

/// LLM settings state
#[derive(Debug, Clone)]
struct LLMSettingsState {
    servers: Vec<LLMServerConfig>,
    available_models: Vec<LLMModel>,
    new_server_url: String,
    new_server_provider: String,
}

/// Dock item for navigation
#[derive(Debug, Clone)]
struct DockItem {
    name: String,
    icon: String,
    action: Message,
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
    
    // Component states
    agents: Vec<Agent>,
    selected_agent: Option<String>,
    logs: Vec<String>,
    config_state: AgentConfigState,
    llm_settings: LLMSettingsState,
    dock_items: Vec<DockItem>,
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
        // Define default dock items
        let dock_items = vec![
            DockItem {
                name: "Agents".to_string(),
                icon: "👥".to_string(),
                action: Message::ChangeView(View::Agents),
            },
            DockItem {
                name: "Settings".to_string(),
                icon: "⚙️".to_string(),
                action: Message::ChangeView(View::Settings),
            },
            DockItem {
                name: "Logs".to_string(),
                icon: "📝".to_string(),
                action: Message::ChangeView(View::Logs),
            },
            DockItem {
                name: "Workflows".to_string(),
                icon: "🔄".to_string(),
                action: Message::ChangeView(View::Workflows),
            },
            DockItem {
                name: "Tasks".to_string(),
                icon: "✅".to_string(),
                action: Message::ChangeView(View::Tasks),
            },
        ];

        (
            Example {
                cli_handler: std::sync::Arc::new(crate::cli::CliHandler::new()),
                current_view: View::Agents,
                agents: Vec::new(),
                selected_agent: None,
                logs: Vec::new(),
                config_state: AgentConfigState::default(),
                llm_settings: LLMSettingsState {
                    servers: Vec::new(),
                    available_models: Vec::new(),
                    new_server_url: String::new(),
                    new_server_provider: String::new(),
                },
                dock_items,
            },
            Task::none(),
        )
    }

    fn title(&self) -> &'static str {
        "Nexa Agent Management"
    }

    fn update(&mut self, message: Message) -> Task<Message> {
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
                        self.llm_settings.servers.push(LLMServerConfig {
                            url,
                            provider,
                        });
                        self.llm_settings.new_server_url.clear();
                        self.llm_settings.new_server_provider.clear();
                        Task::none()
                    }
                    settings::SettingsMessage::RemoveServer(provider) => {
                        self.llm_settings.servers.retain(|s| s.provider != provider);
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
                        self.logs.clear();
                        Task::none()
                    }
                    logs::LogMessage::Show(log) => {
                        self.logs.push(log);
                        if self.logs.len() > MAX_LOGS {
                            self.logs.remove(0);
                        }
                        Task::none()
                    }
                }
            }
            Message::ChangeView(view) => {
                self.current_view = view;
                Task::none()
            }
            Message::Tick => {
                // Periodic updates
                Task::none()
            }
            Message::Batch(messages) => {
                let mut last_task = Task::none();
                for message in messages {
                    last_task = self.update(message);
                }
                last_task
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let dock = self.view_dock();
        let content = match self.current_view {
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
            iced::time::every(REFRESH_INTERVAL)
                .map(|_| Message::Tick),
        ])
    }
}

impl Default for Example {
    fn default() -> Self {
        Example::new().0
    }
}