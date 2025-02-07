use iced::keyboard;
use log::debug;
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    button, column, container, row, scrollable, text, Text, Row,
    text_input,
};
use iced::{Element, Length, Size, Subscription, Task};
use crate::models::agent::Agent;
use crate::cli::AgentConfig;
use iced::Theme;
use std::time::Duration;
use iced::time;

// Constants for UI configuration
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const LOG_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const MAX_LOGS: usize = 1000;
const DEFAULT_WINDOW_SIZE: Size = Size::new(1920.0, 1080.0);

pub fn main() -> iced::Result {
    let example = Example::new().0;
    iced::application(example.title(), Example::update, Example::view)
        .subscription(Example::subscription)
        .window_size(DEFAULT_WINDOW_SIZE)
        .theme(|_| iced::Theme::Light)
        .run()
}

#[derive(Debug, Clone)]
pub enum View {
    Agents,
    Settings,
    Logs,
}

#[derive(Debug, Clone)]
struct LLMSettingsState {
    servers: Vec<LLMServerConfig>,
    selected_provider: String,
    available_models: Vec<LLMModel>,
    new_server_url: String,
    new_server_provider: String,
}

#[derive(Debug, Clone)]
struct LLMServerConfig {
    provider: String,
    url: String,
    is_connected: bool,
    selected_model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LLMModel {
    pub name: String,
    pub size: String,
    pub context_length: usize,
    pub quantization: Option<String>,
    pub description: String,
}

struct Example {
    panes: pane_grid::State<Pane>,
    panes_created: usize,
    focus: Option<pane_grid::Pane>,
    cli_handler: std::sync::Arc<crate::cli::CliHandler>,
    agents: Vec<Agent>,
    selected_agent: Option<String>,
    logs: Vec<String>,
    config_state: AgentConfigState,
    search_query: String,
    sort_order: SortOrder,
    dock_items: Vec<DockItem>,
    llm_settings: LLMSettingsState,
    current_view: View,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Layout Messages
    Split(pane_grid::Axis, pane_grid::Pane),
    SplitFocused(pane_grid::Axis),
    FocusAdjacent(pane_grid::Direction),
    Clicked(pane_grid::Pane),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    TogglePin(pane_grid::Pane),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
    CloseFocused,
    
    // CLI Messages
    CliStart,
    CliStop,
    CliStatus,
    
    // Agent Management
    RefreshAgents,
    StartAgent(String),
    StopAgent(String),
    ViewAgentDetails(String),
    AgentsUpdated(Vec<Agent>),
    UpdateAgentConfig(String, AgentConfig),
    
    // Configuration
    UpdateConfig(ConfigMessage),
    SaveConfig(String),
    ConfigUpdate(ConfigMessage),
    
    // UI State
    ChangeView(View),
    UpdateSearch(String),
    UpdateSort(SortOrder),
    SearchQueryChanged(String),
    SortOrderChanged(SortOrder),
    
    // LLM Management
    AddLLMServer(String, String),  // (url, provider)
    RemoveLLMServer(String),
    ConnectLLM(String),
    DisconnectLLM(String),
    ConnectToLLM(String),
    DisconnectFromLLM(String),
    UpdateNewServerUrl(String),
    UpdateNewServerProvider(String),
    SelectModel(String, String),
    ModelsLoaded(String, Vec<LLMModel>),
    
    // System
    ShowLog(String),
    ShowLogs(String),
    ClearLogs,
    Batch(Vec<Message>),
}

// New helper types for better code organization
#[derive(Debug, Clone)]
pub enum AgentAction {
    Start(String),
    Stop(String),
    Update(String, AgentConfig),
}

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

impl Example {
    fn new() -> (Self, Task<Message>) {
        let (panes, _) = pane_grid::State::new(Pane::new(0));

        // Define default dock items
        let dock_items = vec![
            DockItem {
                name: "Agents".to_string(),
                icon: "👥",
                action: Message::ChangeView(View::Agents),
            },
            DockItem {
                name: "Settings".to_string(),
                icon: "⚙️",
                action: Message::ChangeView(View::Settings),
            },
            DockItem {
                name: "Logs".to_string(),
                icon: "📝",
                action: Message::ChangeView(View::Logs),
            },
        ];

        (
        Example {
            panes,
            panes_created: 1,
            focus: None,
            cli_handler: std::sync::Arc::new(crate::cli::CliHandler::new()),
            agents: Vec::new(),
            selected_agent: None,
            logs: Vec::new(),
                config_state: AgentConfigState::default(),
                search_query: String::new(),
                sort_order: SortOrder::NameAsc,
                dock_items,
                llm_settings: LLMSettingsState {
                    servers: Vec::new(),
                    selected_provider: String::new(),
                    available_models: Vec::new(),
                    new_server_url: String::new(),
                    new_server_provider: String::new(),
                },
                current_view: View::Agents,
            },
            Task::none(),
        )
    }

    fn title(&self) -> &'static str {
        "Nexa Agent Management"
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Split(axis, pane) => {
                let result =
                    self.panes.split(axis, pane, Pane::new(self.panes_created));

                if let Some((pane, _)) = result {
                    self.focus = Some(pane);
                }

                self.panes_created += 1;
                Task::none()
            }
            Message::SplitFocused(axis) => {
                if let Some(pane) = self.focus {
                    let result = self.panes.split(
                        axis,
                        pane,
                        Pane::new(self.panes_created),
                    );

                    if let Some((pane, _)) = result {
                        self.focus = Some(pane);
                    }

                    self.panes_created += 1;
                }
                Task::none()
            }
            Message::FocusAdjacent(direction) => {
                if let Some(pane) = self.focus {
                    if let Some(adjacent) = self.panes.adjacent(pane, direction)
                    {
                        self.focus = Some(adjacent);
                    }
                }
                Task::none()
            }
            Message::Clicked(pane) => {
                self.focus = Some(pane);
                Task::none()
            }
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Task::none()
            }
            Message::Dragged(pane_grid::DragEvent::Dropped {
                pane,
                target,
            }) => {
                self.panes.drop(pane, target);
                Task::none()
            }
            Message::Dragged(_) => Task::none(),
            Message::TogglePin(pane) => {
                if let Some(Pane { is_pinned, .. }) = self.panes.get_mut(pane) {
                    *is_pinned = !*is_pinned;
                }
                Task::none()
            }
            Message::Maximize(pane) => {
                self.panes.maximize(pane);
                Task::none()
            }
            Message::Restore => {
                self.panes.restore();
                Task::none()
            }
            Message::Close(pane) => {
                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focus = Some(sibling);
                }
                Task::none()
            }
            Message::CloseFocused => {
                if let Some(pane) = self.focus {
                    if let Some(Pane { is_pinned, .. }) = self.panes.get(pane) {
                        if !is_pinned {
                            if let Some((_, sibling)) = self.panes.close(pane) {
                                self.focus = Some(sibling);
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::CliStart => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.start(None).await {
                        eprintln!("Failed to start server: {:?}", e);
                    }
                });
                Task::none()
            }
            Message::CliStop => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.stop().await {
                        eprintln!("Failed to stop server: {:?}", e);
                    }
                });
                Task::none()
            }
            Message::CliStatus => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.status().await {
                        eprintln!("Failed to get status: {:?}", e);
                    }
                });
                Task::none()
            }
            Message::RefreshAgents => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Ok(cli_agents) = handler.list_agents(None).await {
                        debug!("Found {} agents", cli_agents.len());
                        let agents = cli_agents.into_iter()
                            .map(crate::models::agent::Agent::from_cli_agent)
                            .collect();
                        return Message::AgentsUpdated(agents);
                    }
                    Message::AgentsUpdated(Vec::new()) // Return empty list on error
                });
                Task::none()
            }
            Message::StartAgent(id) => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.create_agent(
                        format!("Agent {}", id),
                        crate::cli::AgentConfig::default()
                    ).await {
                        eprintln!("Failed to start agent: {}", e);
                    }
                });
                Task::none()
            }
            Message::StopAgent(id) => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.stop_agent(&id).await {
                        eprintln!("Failed to stop agent: {}", e);
                    }
                });
                Task::none()
            }
            Message::ViewAgentDetails(id) => {
                if id.is_empty() {
                    self.selected_agent = None;
                    Task::none()
                } else {
                    self.selected_agent = Some(id.clone());
                    let handler = self.cli_handler.clone();
                    Task::perform(
                        async move {
                            if let Ok(config) = handler.get_agent_config(&id).await {
                                vec![
                                    Message::ConfigUpdate(ConfigMessage::UpdateMaxTasks(config.max_concurrent_tasks.to_string())),
                                    Message::ConfigUpdate(ConfigMessage::UpdatePriority(config.priority_threshold.to_string())),
                                    Message::ConfigUpdate(ConfigMessage::UpdateProvider(config.llm_provider.clone())),
                                    Message::ConfigUpdate(ConfigMessage::UpdateModel(config.llm_model.clone())),
                                    Message::ConfigUpdate(ConfigMessage::UpdateTimeout(config.timeout_seconds.to_string())),
                                    Message::ShowLogs(format!("Loaded configuration for agent {}", id)),
                                ]
                            } else {
                                vec![Message::ShowLogs(format!("Failed to load configuration for agent {}", id))]
                            }
                        },
                        Message::Batch
                    )
                }
            }
            Message::ConnectLLM(_id) => {
                // Implementation for connecting to an LLM
                Task::none()
            }
            Message::DisconnectLLM(provider) => {
                if let Some(server) = self.llm_settings.servers.iter_mut()
                    .find(|s| s.provider == provider) {
                    server.is_connected = false;
                }
                self.llm_settings.selected_provider = String::new();
                Task::none()
            }
            Message::AgentsUpdated(new_agents) => {
                self.agents = new_agents;
                Task::none()
            }
            Message::UpdateAgentConfig(id, config) => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.update_agent_config(id, config).await {
                        eprintln!("Failed to update agent config: {}", e);
                    }
                });
                Task::none()
            }
            Message::ShowLogs(log) => {
                self.logs.push(log);
                // Keep only the last 1000 log entries to prevent memory issues
                if self.logs.len() > 1000 {
                    self.logs.drain(0..self.logs.len() - 1000);
                }
                Task::none()
            }
            Message::ClearLogs => {
                self.logs.clear();
                Task::none()
            }
            Message::ConfigUpdate(msg) => {
                match msg {
                    ConfigMessage::UpdateMaxTasks(value) => {
                        self.config_state.max_concurrent_tasks = value.clone();
                        if let Ok(tasks) = value.parse() {
                            if let Some(agent_id) = &self.selected_agent {
                                let mut config = crate::cli::AgentConfig::default();
                                config.max_concurrent_tasks = tasks;
                                let handler = self.cli_handler.clone();
                                let id = agent_id.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handler.update_agent_config(id, config).await {
                                        eprintln!("Failed to update agent config: {}", e);
                                    }
                                });
                            }
                        }
                        Task::none()
                    }
                    ConfigMessage::UpdatePriority(value) => {
                        self.config_state.priority_threshold = value.clone();
                        if let Ok(priority) = value.parse() {
                            if let Some(agent_id) = &self.selected_agent {
                                let mut config = crate::cli::AgentConfig::default();
                                config.priority_threshold = priority;
                                let handler = self.cli_handler.clone();
                                let id = agent_id.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handler.update_agent_config(id, config).await {
                                        eprintln!("Failed to update agent config: {}", e);
                                    }
                                });
                            }
                        }
                        Task::none()
                    }
                    ConfigMessage::UpdateProvider(value) => {
                        self.config_state.llm_provider = value.clone();
                        if let Some(agent_id) = &self.selected_agent {
                            let mut config = crate::cli::AgentConfig::default();
                            config.llm_provider = value;
                            let handler = self.cli_handler.clone();
                            let id = agent_id.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handler.update_agent_config(id, config).await {
                                    eprintln!("Failed to update agent config: {}", e);
                                }
                            });
                        }
                        Task::none()
                    }
                    ConfigMessage::UpdateModel(value) => {
                        self.config_state.llm_model = value.clone();
                        if let Some(agent_id) = &self.selected_agent {
                            let mut config = crate::cli::AgentConfig::default();
                            config.llm_model = value;
                            let handler = self.cli_handler.clone();
                            let id = agent_id.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handler.update_agent_config(id, config).await {
                                    eprintln!("Failed to update agent config: {}", e);
                                }
                            });
                        }
                        Task::none()
                    }
                    ConfigMessage::UpdateTimeout(value) => {
                        self.config_state.timeout_seconds = value.clone();
                        if let Ok(timeout) = value.parse() {
                            if let Some(agent_id) = &self.selected_agent {
                                let mut config = crate::cli::AgentConfig::default();
                                config.timeout_seconds = timeout;
                                let handler = self.cli_handler.clone();
                                let id = agent_id.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handler.update_agent_config(id, config).await {
                                        eprintln!("Failed to update agent config: {}", e);
                                    }
                                });
                            }
                        }
                        Task::none()
                    }
                }
            }
            Message::Batch(messages) => {
                let mut command = Task::none();
                for message in messages {
                    command = Task::batch(vec![command, self.update(message)]);
                }
                command
            }
            Message::SearchQueryChanged(query) => {
                self.search_query = query;
                Task::none()
            }
            Message::SortOrderChanged(order) => {
                self.sort_order = order;
                Task::none()
            }
            Message::AddLLMServer(url, provider) => {
                self.llm_settings.servers.push(LLMServerConfig {
                    provider: provider.clone(),
                    url: url.clone(),
                    is_connected: false,
                    selected_model: None,
                });
                Task::none()
            }
            Message::RemoveLLMServer(provider) => {
                self.llm_settings.servers.retain(|server| server.provider != provider);
                Task::none()
            }
            Message::ConnectToLLM(provider) => {
                self.llm_settings.selected_provider = provider.clone();
                Task::none()
            }
            Message::DisconnectFromLLM(provider) => {
                if let Some(server) = self.llm_settings.servers.iter_mut()
                    .find(|s| s.provider == provider) {
                    server.is_connected = false;
                }
                self.llm_settings.selected_provider = String::new();
                Task::none()
            }
            Message::SelectModel(model, provider) => {
                if let Some(server) = self.llm_settings.servers.iter_mut()
                    .find(|s| s.provider == provider) {
                    server.selected_model = Some(model.clone());
                }
                
                let handler = self.cli_handler.clone();
                let provider_clone = provider.clone();
                let model_clone = model.clone();
                
                Task::perform(
                    async move {
                        if let Err(e) = handler.select_model(&provider_clone, &model_clone).await {
                            Message::ShowLogs(format!("Failed to select model: {}", e))
                        } else {
                            Message::ShowLogs(format!("Selected model {} for {}", model, provider))
                        }
                    },
                    |msg| msg
                )
            }
            Message::UpdateNewServerUrl(url) => {
                self.llm_settings.new_server_url = url.clone();
                Task::none()
            }
            Message::UpdateNewServerProvider(provider) => {
                self.llm_settings.new_server_provider = provider.clone();
                Task::none()
            }
            Message::ModelsLoaded(_provider, models) => {
                self.llm_settings.available_models = models;
                Task::none()
            }
            Message::ChangeView(view) => {
                self.current_view = view;
                Task::none()
            }
            Message::UpdateConfig(_config) => {
                // Handle config update
                Task::none()
            }
            Message::SaveConfig(_id) => {
                // Handle config save
                Task::none()
            }
            Message::UpdateSearch(query) => {
                self.search_query = query;
                Task::none()
            }
            Message::UpdateSort(order) => {
                self.sort_order = order;
                Task::none()
            }
            Message::ShowLog(log) => {
                self.logs.push(log);
                if self.logs.len() > MAX_LOGS {
                    self.logs.drain(0..self.logs.len() - MAX_LOGS);
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let content = match self.current_view {
            View::Agents => self.view_agent_panel(),
            View::Settings => self.view_settings(),
            View::Logs => self.view_logs_panel(),
        };

        let dock_items: Vec<Element<Message>> = self.dock_items.iter().map(|item| {
            let button: Element<Message> = button(
                container(
                    column![
                        Text::new(item.icon).size(32),
                        Text::new(&item.name).size(12),
                    ]
                    .spacing(5)
                )
                .padding(10)
                .style(style::dock_item)
            )
            .on_press(item.action.clone())
            .style(if let Message::ChangeView(ref view) = item.action {
                if std::mem::discriminant(view) == std::mem::discriminant(&self.current_view) {
                    button::primary
                } else {
                    button::secondary
                }
            } else {
                button::secondary
            })
            .into();
            button
        }).collect();

        let dock_buttons: Element<Message> = Row::with_children(dock_items).spacing(15).into();

        let dock_container: Element<Message> = container(dock_buttons)
            .padding(10)
            .style(style::dock)
            .into();

        let column_content: Element<Message> = column![
            content,
            dock_container,
        ].into();

        container(column_content)
            .padding(20)
            .style(style::main_container)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            keyboard::on_key_press(|key_code, modifiers| {
                if !modifiers.command() {
                    return None;
                }
                handle_hotkey(key_code)
            }),
            time::every(REFRESH_INTERVAL)
                .map(|_| Message::RefreshAgents),
            self.log_subscription(),
        ])
    }

    fn log_subscription(&self) -> Subscription<Message> {
        time::every(LOG_CHECK_INTERVAL)
            .map(|_| {
                let now = std::time::Instant::now();
                Message::ShowLogs(format!("System check at: {:?}", now))
            })
    }

    #[allow(dead_code)]
    fn view_pane_grid(&self) -> Element<Message> {
        let pane_grid = PaneGrid::new(
            &self.panes,
            |_id, pane, _is_maximized| {
                let title = format!("Pane {}", pane.id);
                let content: Element<_> = text(title).into();
                pane_grid::Content::new(content)
            }
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(10)
        .on_drag(Message::Dragged)
        .on_resize(10, Message::Resized);

        container(pane_grid)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .into()
    }

    fn view_settings(&self) -> Element<Message> {
        let header = container(
            Text::new("LLM Settings")
                .size(32)
                .style(style::header_text)
        )
        .padding(20)
        .style(style::panel_content);

        let add_server_form = container(
            column![
                Text::new("Add New LLM Server")
                    .size(24)
                    .style(style::header_text),
                row![
                    text_input("Provider (e.g. LMStudio, Ollama)", &self.llm_settings.new_server_provider)
                        .on_input(Message::UpdateNewServerProvider)
                        .padding(10)
                        .size(16),
                    text_input("Server URL", &self.llm_settings.new_server_url)
                        .on_input(Message::UpdateNewServerUrl)
                        .padding(10)
                        .size(16),
                    button(Text::new("Add Server").size(16))
                        .on_press(Message::AddLLMServer(
                            self.llm_settings.new_server_provider.clone(),
                            self.llm_settings.new_server_url.clone()
                        ))
                        .padding(10)
                        .style(button::primary),
                ].spacing(15),
            ]
            .spacing(20)
        )
        .padding(20)
        .style(style::panel_content);

        let servers_list = container(
            column(
                self.llm_settings.servers.iter().map(|server| {
                    container(
                        column![
                            row![
                                Text::new(&server.provider).size(20).style(style::header_text),
                                Text::new(&server.url).size(16),
                                if server.is_connected {
                                    button(Text::new("Disconnect").size(16))
                                        .on_press(Message::DisconnectFromLLM(server.provider.clone()))
                                        .style(button::danger)
                                } else {
                                    button(Text::new("Connect").size(16))
                                        .on_press(Message::ConnectToLLM(server.provider.clone()))
                                        .style(button::primary)
                                },
                                button(Text::new("Remove").size(16))
                                    .on_press(Message::RemoveLLMServer(server.provider.clone()))
                                    .style(button::danger),
                            ].spacing(15),
                            if server.is_connected {
                                container(
                                    column![
                                        Text::new("Available Models").size(16).style(style::header_text),
                                        column(
                                            self.llm_settings.available_models.iter().map(|model| {
                                                container(
                                                    row![
                                                        column![
                                                            Text::new(&model.name).size(16),
                                                            Text::new(&model.description).size(14),
                                                        ],
                                                        Text::new(format!("Size: {}", model.size)).size(14),
                                                        Text::new(format!("Context: {}k tokens", model.context_length / 1024)).size(14),
                                                        if let Some(quant) = &model.quantization {
                                                            Text::new(format!("Quantization: {}", quant)).size(14)
                                                        } else {
                                                            Text::new("").size(14)
                                                        },
                                                        button(
                                                            Text::new(
                                                                if Some(&model.name) == server.selected_model.as_ref() {
                                                                    "Selected"
                                                                } else {
                                                                    "Select"
                                                                }
                                                            ).size(14)
                                                        )
                                                        .on_press(Message::SelectModel(
                                                            server.provider.clone(),
                                                            model.name.clone()
                                                        ))
                                                        .style(
                                                            if Some(&model.name) == server.selected_model.as_ref() {
                                                                button::primary
                                                            } else {
                                                                button::secondary
                                                            }
                                                        ),
                                                    ]
                                                    .spacing(15)
                                                    .align_y(iced::alignment::Vertical::Center)
                                                )
                                                .padding(10)
                                                .style(style::panel_content)
                                                .into()
                                            }).collect::<Vec<Element<Message>>>()
                                        ).spacing(10)
                                    ]
                                ).padding(10)
                                .into()
                            } else {
                                let content: Element<Message> = container(
                                    Text::new("Connect to view available models")
                                        .size(14)
                                ).padding(10)
                                 .style(style::panel_content)
                                 .into();
                                content
                            }
                        ]
                        .spacing(15)
                    )
                    .padding(15)
                    .style(style::panel_content)
                    .into()
                }).collect::<Vec<Element<Message>>>()
            ).spacing(20)
        )
        .padding(20)
        .style(style::panel_content);

        container(
            column![
                header,
                add_server_form,
                servers_list,
            ]
            .spacing(20)
        )
        .padding(20)
        .into()
    }

    fn view_logs_panel(&self) -> Element<Message> {
        container(
            column![
                Text::new("System Logs")
                    .size(32)
                    .style(style::header_text),
                scrollable(
                    column(
                        self.logs.iter()
                            .map(|log| Text::new(log).size(14).into())
                            .collect::<Vec<Element<Message>>>()
                    ).spacing(10)
                ).height(Length::Fill),
                button(
                    Text::new("Clear Logs")
                        .size(16)
                )
                .on_press(Message::ClearLogs)
                .padding(15)
                .style(button::danger),
            ]
            .spacing(20)
        )
        .padding(20)
        .style(style::panel_content)
        .into()
    }

    fn view_agent_panel(&self) -> Element<Message> {
        match &self.selected_agent {
            Some(_) => self.view_agent_details(),
            None => {
                let header = container(
                    Text::new("AI Agents Management")
                        .size(32)
                        .style(style::header_text)
                )
                .padding(20)
                .style(style::panel_content);

                // Add search input with new styling
                let search_bar = container(
                    row![
                        Text::new("🔍").size(20),
                        text_input("Search agents...", &self.search_query)
                            .on_input(Message::SearchQueryChanged)
                            .padding(10)
                            .size(16)
                            .width(Length::Fill),
                    ]
                    .spacing(15)
                )
                .padding(15)
                .style(style::search_bar);

                let sort_controls = container(
                    row![
                        Text::new("Sort by:").size(16).style(style::header_text),
                        button(Text::new("Name").size(16))
                            .on_press(Message::SortOrderChanged(
                                if self.sort_order == SortOrder::NameAsc {
                                    SortOrder::NameDesc
                                } else {
                                    SortOrder::NameAsc
                                }
                            ))
                            .padding(10)
                            .style(if matches!(self.sort_order, SortOrder::NameAsc | SortOrder::NameDesc) {
                                button::primary
                            } else {
                                button::secondary
                            }),
                        button(Text::new("Status").size(16))
                            .on_press(Message::SortOrderChanged(
                                if self.sort_order == SortOrder::StatusAsc {
                                    SortOrder::StatusDesc
                                } else {
                                    SortOrder::StatusAsc
                                }
                            ))
                            .padding(10)
                            .style(if matches!(self.sort_order, SortOrder::StatusAsc | SortOrder::StatusDesc) {
                                button::primary
                            } else {
                                button::secondary
                            }),
                        button(Text::new("Last Active").size(16))
                            .on_press(Message::SortOrderChanged(
                                if self.sort_order == SortOrder::LastActiveAsc {
                                    SortOrder::LastActiveDesc
                                } else {
                                    SortOrder::LastActiveAsc
                                }
                            ))
                            .padding(10)
                            .style(if matches!(self.sort_order, SortOrder::LastActiveAsc | SortOrder::LastActiveDesc) {
                                button::primary
                            } else {
                                button::secondary
                            }),
                    ]
                    .spacing(15)
                )
                .padding(20)
                .style(style::panel_content);

                let mut agent_list = column![header, search_bar, sort_controls].spacing(20);

                // Filter and sort agents
                let mut filtered_agents: Vec<_> = self.agents.iter()
                    .filter(|agent| {
                        if self.search_query.is_empty() {
                            true
                        } else {
                            agent.name.to_lowercase().contains(&self.search_query.to_lowercase()) ||
                            agent.id.to_lowercase().contains(&self.search_query.to_lowercase())
                        }
                    })
                    .collect();

                filtered_agents.sort_by(|a, b| {
                    match self.sort_order {
                        SortOrder::NameAsc => a.name.cmp(&b.name),
                        SortOrder::NameDesc => b.name.cmp(&a.name),
                        SortOrder::StatusAsc => a.status.cmp(&b.status),
                        SortOrder::StatusDesc => b.status.cmp(&a.status),
                        SortOrder::LastActiveAsc => a.last_heartbeat.cmp(&b.last_heartbeat),
                        SortOrder::LastActiveDesc => b.last_heartbeat.cmp(&a.last_heartbeat),
                    }
                });

                for agent in filtered_agents {
                    let agent_row = container(
                        row![
                            container(
                                Text::new("●")
                                    .size(12)
                            )
                            .padding(5)
                            .style(|theme| style::status_badge_style(agent.status)(theme)),
                        Text::new(&agent.name).size(16),
                            row![
                                button(Text::new("Start").size(14))
                            .on_press(Message::StartAgent(agent.id.clone()))
                                .padding(10)
                            .style(button::primary),
                                button(Text::new("Stop").size(14))
                            .on_press(Message::StopAgent(agent.id.clone()))
                                .padding(10)
                            .style(button::danger),
                                button(Text::new("Details").size(14))
                                    .on_press(Message::ViewAgentDetails(agent.id.clone()))
                                    .padding(10),
                            ].spacing(10)
                        ]
                        .spacing(15)
                        .align_y(iced::alignment::Vertical::Center)
                    )
                    .padding(15)
                    .style(style::panel_content);

                    agent_list = agent_list.push(agent_row);
                }

                let logs_section = container(
                    column![
                        Text::new("System Logs").size(20).style(style::header_text),
                        scrollable(
                            column(
                                self.logs.iter()
                                    .map(|log| Text::new(log).size(14).into())
                                    .collect::<Vec<Element<Message>>>()
                            ).spacing(10)
                        ).height(Length::Fixed(200.0))
                    ]
                )
                .padding(20)
                .style(style::panel_content);

                let refresh_button = button(
                    Text::new("Refresh Agents")
                        .size(16)
                )
                    .on_press(Message::RefreshAgents)
                .padding(15)
                .style(button::primary);

                container(
                    column![
                        agent_list,
                        logs_section,
                        refresh_button,
                    ]
                    .spacing(20)
                )
                    .padding(20)
                .into()
            }
        }
    }

    fn view_agent_details(&self) -> Element<Message> {
        if let Some(agent_id) = &self.selected_agent {
            if let Some(agent) = self.agents.iter().find(|a| &a.id == agent_id) {
                let header = container(
                    Text::new(format!("Agent Details: {}", agent.name))
                        .size(32)
                        .style(style::header_text)
                )
                .padding(20)
                .style(style::panel_content);

                let details = container(
                    column![
                        row![
                            container(
                                Text::new("●")
                                    .size(12)
                            )
                            .padding(5)
                            .style(|theme| style::status_badge_style(agent.status)(theme)),
                            Text::new(format!("Status: {:?}", agent.status))
                                .size(16)
                                .style(style::header_text)
                        ].spacing(10),
                        row![
                            Text::new("ID: ").size(16).style(style::header_text),
                            Text::new(&agent.id).size(16)
                        ],
                        row![
                            Text::new("Capabilities: ").size(16).style(style::header_text),
                            Text::new(agent.capabilities.join(", ")).size(16)
                        ],
                        row![
                            Text::new("Last Heartbeat: ").size(16).style(style::header_text),
                            Text::new(agent.last_heartbeat.to_string()).size(16)
                        ],
                        if let Some(task) = &agent.current_task {
                            row![
                                Text::new("Current Task: ").size(16).style(style::header_text),
                                Text::new(task).size(16)
                            ]
                        } else {
                            row![
                                Text::new("Current Task: ").size(16).style(style::header_text),
                                Text::new("None").size(16)
                            ]
                        },
                    ]
                    .spacing(15)
                )
                .padding(20)
                .style(style::panel_content);

                let config_form = container(
                    column![
                        Text::new("Agent Configuration")
                            .size(24)
                            .style(style::header_text),
                        column![
                            row![
                                Text::new("Max Concurrent Tasks: ").size(16),
                                text_input(
                                    "4",
                                    &self.config_state.max_concurrent_tasks,
                                )
                                .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdateMaxTasks(value)))
                                .padding(10)
                                .size(16)
                            ],
                            row![
                                Text::new("Priority Threshold: ").size(16),
                                text_input(
                                    "0",
                                    &self.config_state.priority_threshold,
                                )
                                .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdatePriority(value)))
                                .padding(10)
                                .size(16)
                            ],
                            row![
                                Text::new("LLM Provider: ").size(16),
                                text_input(
                                    "LMStudio",
                                    &self.config_state.llm_provider,
                                )
                                .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdateProvider(value)))
                                .padding(10)
                                .size(16)
                            ],
                            row![
                                Text::new("LLM Model: ").size(16),
                                text_input(
                                    "default",
                                    &self.config_state.llm_model,
                                )
                                .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdateModel(value)))
                                .padding(10)
                                .size(16)
                            ],
                            row![
                                Text::new("Timeout (seconds): ").size(16),
                                text_input(
                                    "30",
                                    &self.config_state.timeout_seconds,
                                )
                                .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdateTimeout(value)))
                                .padding(10)
                                .size(16)
                            ],
                        ].spacing(15)
                    ]
                    .spacing(20)
                )
                .padding(20)
                .style(style::panel_content);

                let logs = self.logs.iter()
                    .filter(|log| log.contains(&agent.id))
                    .collect::<Vec<_>>();

                let log_view = container(
                    column![
                        Text::new("Agent Logs")
                            .size(24)
                            .style(style::header_text),
                        scrollable(
                            column(
                                logs.iter()
                                    .map(|log| Text::new(log.as_str()).size(14).into())
                                    .collect::<Vec<Element<Message>>>()
                            ).spacing(10)
                        ).height(Length::Fixed(200.0))
                    ]
                )
                .padding(20)
                .style(style::panel_content);

                let close_button = button(
                    Text::new("Close Details")
                        .size(16)
                )
                .on_press(Message::ViewAgentDetails(String::new()))
                .padding(15)
                .style(button::primary);

                container(
                    column![
                        header,
                        details,
                        config_form,
                        log_view,
                        close_button,
                    ]
                    .spacing(20)
                )
                .padding(20)
                .style(style::panel_content)
                .into()
            } else {
                container(
                    Text::new("Agent not found")
                        .size(32)
                        .style(style::header_text)
                )
                .padding(20)
                .style(style::panel_content)
                .into()
            }
        } else {
            container(
                Text::new("Select an agent to view details")
                    .size(32)
                    .style(style::header_text)
            )
            .padding(20)
            .style(style::panel_content)
            .into()
        }
    }
}

#[derive(Debug, Clone)]
struct AgentConfigState {
    max_concurrent_tasks: String,
    priority_threshold: String,
    llm_provider: String,
    llm_model: String,
    timeout_seconds: String,
}

impl Default for AgentConfigState {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: "4".to_string(),
            priority_threshold: "0".to_string(),
            llm_provider: "LMStudio".to_string(),
            llm_model: "default".to_string(),
            timeout_seconds: "30".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfigMessage {
    UpdateMaxTasks(String),
    UpdatePriority(String),
    UpdateProvider(String),
    UpdateModel(String),
    UpdateTimeout(String),
}

impl Default for Example {
    fn default() -> Self {
        Example::new().0
    }
}

fn handle_hotkey(key: keyboard::Key) -> Option<Message> {
    use keyboard::key::{self, Key};
    use pane_grid::{Axis, Direction};

    match key.as_ref() {
        Key::Character("v") => Some(Message::SplitFocused(Axis::Vertical)),
        Key::Character("h") => Some(Message::SplitFocused(Axis::Horizontal)),
        Key::Character("w") => Some(Message::CloseFocused),
        Key::Named(key) => {
            let direction = match key {
                key::Named::ArrowUp => Some(Direction::Up),
                key::Named::ArrowDown => Some(Direction::Down),
                key::Named::ArrowLeft => Some(Direction::Left),
                key::Named::ArrowRight => Some(Direction::Right),
                _ => None,
            };

            direction.map(Message::FocusAdjacent)
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct Pane {
    id: usize,
    pub is_pinned: bool,
}

impl Pane {
    fn new(id: usize) -> Self {
        Self {
            id,
            is_pinned: false,
        }
    }
}

mod style {
    use super::*;
    use iced::{Border, Color, Shadow};
    use iced::widget::container;
    use crate::models::agent::AgentStatus;

    // Modern color palette with semantic naming
    pub struct ThemeColors {
        pub background: Color,
        pub surface: Color,
        pub border: Color,
        pub text: Color,
    }

    impl ThemeColors {
        pub fn light() -> Self {
            Self {
                background: Color::from_rgb(0.98, 0.99, 1.00),
                surface: Color::from_rgb(1.0, 1.0, 1.0),
                border: Color::from_rgb(0.90, 0.92, 0.95),
                text: Color::from_rgb(0.15, 0.18, 0.20),
            }
        }
    }

    // Reusable shadow definitions
    pub struct Shadows {
        pub small: Shadow,
        pub medium: Shadow,
        pub large: Shadow,
    }

    impl Shadows {
        pub fn new() -> Self {
            Self {
                small: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.08),
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 8.0,
                },
                medium: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.12),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                large: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.16),
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 24.0,
                },
            }
        }
    }

    // New component-specific styles
    pub fn dock_item(_theme: &Theme) -> container::Style {
        let colors = ThemeColors::light();
        
        container::Style {
            background: Some(colors.surface.into()),
            border: Border {
                width: 1.0,
                color: colors.border,
                radius: (12.0).into(),
            },
            shadow: Shadows::new().medium,
            text_color: Some(colors.text),
            ..Default::default()
        }
    }

    pub fn dock(_theme: &Theme) -> container::Style {
        let colors = ThemeColors::light();
        
        container::Style {
            background: Some(colors.surface.into()),
            border: Border {
                width: 1.0,
                color: colors.border,
                radius: (20.0).into(),
            },
            shadow: Shadows::new().large,
            ..Default::default()
        }
    }

    pub fn main_container(_theme: &Theme) -> container::Style {
        let colors = ThemeColors::light();
        
        container::Style {
            background: Some(colors.background.into()),
            border: Border {
                width: 1.0,
                color: colors.border,
                radius: (16.0).into(),
            },
            shadow: Shadows::new().medium,
            ..Default::default()
        }
    }

    pub fn header_text(_theme: &Theme) -> iced::widget::text::Style {
        let colors = ThemeColors::light();
        
        iced::widget::text::Style {
            color: Some(colors.text),
            ..Default::default()
        }
    }

    pub fn panel_content(_theme: &Theme) -> container::Style {
        let colors = ThemeColors::light();

        container::Style {
            background: Some(colors.surface.into()),
            border: Border {
                width: 1.0,
                color: colors.border,
                radius: (12.0).into(),
            },
            shadow: Shadows::new().small,
            ..Default::default()
        }
    }

    pub fn search_bar(_theme: &Theme) -> container::Style {
        let colors = ThemeColors::light();

        container::Style {
            background: Some(colors.surface.into()),
            border: Border {
                width: 1.0,
                color: colors.border,
                radius: (10.0).into(),
            },
            shadow: Shadows::new().small,
            ..Default::default()
        }
    }

    pub fn status_badge_style(status: AgentStatus) -> impl Fn(&Theme) -> container::Style {
        move |_theme: &Theme| {
            let color = match status {
                AgentStatus::Active => Color::from_rgb(0.2, 0.8, 0.4),  // Green
                AgentStatus::Idle => Color::from_rgb(0.9, 0.7, 0.2),    // Yellow
            };

            container::Style {
                background: Some(color.into()),
                border: Border {
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    radius: (4.0).into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(color.r, color.g, color.b, 0.3),
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 4.0,
                },
                ..Default::default()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortOrder {
    NameAsc,
    NameDesc,
    StatusAsc,
    StatusDesc,
    LastActiveAsc,
    LastActiveDesc,
}

// Add dock item struct
#[derive(Debug, Clone)]
struct DockItem {
    name: String,
    icon: &'static str,
    action: Message,
}