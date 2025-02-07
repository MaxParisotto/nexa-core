use iced::keyboard;
use log::debug;
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    button, column, container, row, scrollable, text, Text, Row,
};  
use iced::{Color, Element, Fill, Length, Size, Subscription, Theme};
use crate::models::agent::{Agent, AgentStatus};

use iced::Task;
use iced::widget::text_input;
use std::time::Duration;
use iced::time;

pub fn main() -> iced::Result {
    let example = Example::new().0;
    iced::application(example.title(), Example::update, Example::view)
        .subscription(Example::subscription)
        .window_size(Size::new(1920.0, 1080.0))
        .theme(|_| iced::Theme::Light)
        .run()
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
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum Message {
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
    CliStart,
    CliStop,
    CliStatus,
    RefreshAgents,
    StartAgent(String),
    StopAgent(String),
    ViewAgentDetails(String),
    ConnectLLM(String),
    DisconnectLLM(String),
    AgentsUpdated(Vec<Agent>),
    UpdateAgentConfig(String, crate::cli::AgentConfig),
    ShowLogs(String),
    ClearLogs,
    ConfigUpdate(ConfigMessage),
    Batch(Vec<Message>),
    SearchQueryChanged(String),
    SortOrderChanged(SortOrder),
}

impl Example {
    fn new() -> (Self, Task<Message>) {
        let (panes, _) = pane_grid::State::new(Pane::new(0));

        // Define default dock items
        let dock_items = vec![
            DockItem {
                name: "Agents".to_string(),
                icon: "👥",
                action: Message::ViewAgentDetails(String::new()),
            },
            DockItem {
                name: "Settings".to_string(),
                icon: "⚙️",
                action: Message::CliStatus,
            },
            DockItem {
                name: "Logs".to_string(),
                icon: "📝",
                action: Message::ClearLogs,
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
            Message::DisconnectLLM(_id) => {
                // Implementation for disconnecting from an LLM
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
        }
    }

    fn view(&self) -> Element<Message> {
        let dock_buttons = Row::with_children(
            self.dock_items.iter().map(|item| {
                button(
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
                .style(button::secondary)
                .into()
            }).collect::<Vec<Element<_>>>()
        ).spacing(15);

        let content = column![
            row![
                container(self.view_agent_panel()).width(Length::FillPortion(1)),
                container(
                    if self.selected_agent.is_some() {
                        self.view_agent_details()
                    } else {
                        self.view_pane_grid()
                    }
                ).width(Length::FillPortion(2)),
            ].spacing(20),
            // Add dock
            container(dock_buttons)
                .padding(10)
                .style(style::dock),
        ];

        container(content)
            .padding(10)
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
            time::every(Duration::from_secs(5))
                .map(|_| Message::RefreshAgents),
            self.log_subscription(),
        ])
    }

    fn log_subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_secs(1))
            .map(|_| {
                let now = std::time::Instant::now();
                Message::ShowLogs(format!("System check at: {:?}", now))
            })
    }

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
enum ConfigMessage {
    UpdateMaxTasks(String),
    UpdatePriority(String),
    UpdateProvider(String),
    UpdateModel(String),
    UpdateTimeout(String),
}

impl Example {
    fn view_agent_panel(&self) -> Element<Message> {
        let header = Text::new("AI Agents Management")
            .size(24)
            .style(style::header_text);

        // Add search input
        let search_bar = container(
            row![
                Text::new("🔍").size(16),
                text_input("Search agents...", &self.search_query)
                    .on_input(Message::SearchQueryChanged)
                    .padding(5)
                    .width(Length::Fill),
            ]
            .spacing(10)
        ).padding(15)
        .style(style::panel_content);

        let sort_controls = container(
            row![
                Text::new("Sort by: ").size(14),
                button(Text::new("Name").size(14))
                    .on_press(Message::SortOrderChanged(
                        if self.sort_order == SortOrder::NameAsc {
                            SortOrder::NameDesc
                        } else {
                            SortOrder::NameAsc
                        }
                    ))
                    .style(if matches!(self.sort_order, SortOrder::NameAsc | SortOrder::NameDesc) {
                        button::primary
                    } else {
                        button::secondary
                    }),
                button(Text::new("Status").size(14))
                    .on_press(Message::SortOrderChanged(
                        if self.sort_order == SortOrder::StatusAsc {
                            SortOrder::StatusDesc
                        } else {
                            SortOrder::StatusAsc
                        }
                    ))
                    .style(if matches!(self.sort_order, SortOrder::StatusAsc | SortOrder::StatusDesc) {
                        button::primary
                    } else {
                        button::secondary
                    }),
                button(Text::new("Last Active").size(14))
                    .on_press(Message::SortOrderChanged(
                        if self.sort_order == SortOrder::LastActiveAsc {
                            SortOrder::LastActiveDesc
                        } else {
                            SortOrder::LastActiveAsc
                        }
                    ))
                    .style(if matches!(self.sort_order, SortOrder::LastActiveAsc | SortOrder::LastActiveDesc) {
                        button::primary
                    } else {
                        button::secondary
                    }),
            ].spacing(10)
        ).padding(15)
        .style(style::panel_content);

        let mut agent_list = column![header, search_bar, sort_controls].spacing(15);

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

        // Sort agents based on current sort order
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
            let status_color = match agent.status {
                AgentStatus::Active => Color::from_rgb(0.0, 0.8, 0.0),
                _ => Color::from_rgb(0.8, 0.0, 0.0),
            };

            let status_indicator = Text::new("●")
                .size(16)
                .style(move |_: &Theme| iced::widget::text::Style {
                    color: Some(status_color),
                    ..Default::default()
                });

            let agent_row = row![
                status_indicator,
                Text::new(&agent.name).size(16),
                button("Start")
                    .on_press(Message::StartAgent(agent.id.clone()))
                    .style(button::primary),
                button("Stop")
                    .on_press(Message::StopAgent(agent.id.clone()))
                    .style(button::danger),
                button("Details")
                    .on_press(Message::ViewAgentDetails(agent.id.clone())),
            ]
            .spacing(10)
            .align_y(iced::alignment::Vertical::Center);

            agent_list = agent_list.push(agent_row);
        }

        let agent_list_container = container(agent_list)
            .padding(15)
            .style(style::panel_content);

        let logs_section = container(
            column![
                Text::new("System Logs").size(16).style(style::header_text),
                scrollable(
                    column(Vec::from_iter(
                        self.logs.iter()
                            .map(|log| Text::new(log.clone()).size(12).into())
                            .collect::<Vec<_>>()
                    ))
                ).height(Length::Fixed(150.0))
            ]
        ).padding(15)
        .style(style::panel_content);

        let refresh_button = button("Refresh")
            .on_press(Message::RefreshAgents)
            .padding(10)
            .style(button::primary);

        container(
            column![
                agent_list_container,
                logs_section,
                refresh_button,
            ]
            .spacing(20)
            .padding(20)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Fill)
        .center_y(Fill)
        .into()
    }

    fn view_agent_details(&self) -> Element<Message> {
        if let Some(agent_id) = &self.selected_agent {
            if let Some(agent) = self.agents.iter().find(|a| &a.id == agent_id) {
                let header = Text::new(format!("Agent Details: {}", agent.name))
                    .size(24)
                    .style(style::header_text);

                let details = container(
                    column![
                        row![
                            Text::new("Status: ").size(16),
                            Text::new(format!("{:?}", agent.status)).size(16)
                        ],
                        row![
                            Text::new("ID: ").size(16),
                            Text::new(&agent.id).size(16)
                        ],
                        row![
                            Text::new("Capabilities: ").size(16),
                            Text::new(agent.capabilities.join(", ")).size(16)
                        ],
                        row![
                            Text::new("Last Heartbeat: ").size(16),
                            Text::new(agent.last_heartbeat.to_string()).size(16)
                        ],
                        if let Some(task) = &agent.current_task {
                            row![
                                Text::new("Current Task: ").size(16),
                                Text::new(task).size(16)
                            ]
                        } else {
                            row![
                                Text::new("Current Task: ").size(16),
                                Text::new("None").size(16)
                            ]
                        },
                    ]
                    .spacing(10)
                    .padding(10)
                ).style(style::panel_content);

                let config_form = container(
                    column![
                        Text::new("Agent Configuration").size(16).style(style::header_text),
                        row![
                            Text::new("Max Concurrent Tasks: ").size(14),
                            text_input(
                                "4",
                                &self.config_state.max_concurrent_tasks,
                            )
                            .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdateMaxTasks(value)))
                            .padding(5)
                        ],
                        row![
                            Text::new("Priority Threshold: ").size(14),
                            text_input(
                                "0",
                                &self.config_state.priority_threshold,
                            )
                            .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdatePriority(value)))
                            .padding(5)
                        ],
                        row![
                            Text::new("LLM Provider: ").size(14),
                            text_input(
                                "LMStudio",
                                &self.config_state.llm_provider,
                            )
                            .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdateProvider(value)))
                            .padding(5)
                        ],
                        row![
                            Text::new("LLM Model: ").size(14),
                            text_input(
                                "default",
                                &self.config_state.llm_model,
                            )
                            .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdateModel(value)))
                            .padding(5)
                        ],
                        row![
                            Text::new("Timeout (seconds): ").size(14),
                            text_input(
                                "30",
                                &self.config_state.timeout_seconds,
                            )
                            .on_input(|value| Message::ConfigUpdate(ConfigMessage::UpdateTimeout(value)))
                            .padding(5)
                        ],
                    ]
                    .spacing(10)
                    .padding(10)
                ).style(style::panel_content);

                let logs = self.logs.iter()
                    .filter(|log| log.contains(&agent.id))
                    .collect::<Vec<_>>();

                let log_view = container(
                    column![
                        Text::new("Agent Logs").size(16),
                        scrollable(
                            column(
                                logs.iter()
                                    .map(|log| Text::new(log.as_str()).size(12).into())
                                    .collect::<Vec<Element<_>>>()
                            )
                        ).height(Length::Fixed(200.0))
                    ]
                ).padding(10)
                .style(style::panel_content);

                container(
                    column![
                        header,
                        details,
                        config_form,
                        log_view,
                        button("Close Details")
                            .on_press(Message::ViewAgentDetails(String::new()))
                            .padding(10)
                    ]
                    .spacing(20)
                )
                .padding(20)
                .into()
            } else {
                container(
                    Text::new("Agent not found")
                        .size(24)
                        .style(style::header_text)
                )
                .padding(20)
                .into()
            }
        } else {
            container(
                Text::new("Select an agent to view details")
                    .size(24)
                    .style(style::header_text)
            )
            .padding(20)
            .into()
        }
    }
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

#[derive(Clone, Copy)]
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
    use iced::widget::container;
    use iced::{Border, Theme, Color, Shadow};

    // Helper function for consistent colors
    fn get_theme_colors(_theme: &Theme) -> (Color, Color, Color, Color) {
        (
            Color::from_rgb(0.98, 0.98, 0.98), // Background
            Color::from_rgb(0.95, 0.95, 0.95), // Secondary Background
            Color::from_rgb(0.85, 0.85, 0.85), // Border
            Color::from_rgb(0.5, 0.5, 0.5),    // Text
        )
    }

    pub fn dock_item(theme: &Theme) -> container::Style {
        let (_, _, border_color, _) = get_theme_colors(theme);
        
        container::Style {
            background: Some(Color::from_rgb(1.0, 1.0, 1.0).into()),
            border: Border {
                width: 1.0,
                color: border_color,
                radius: (8.0).into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.1),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 5.0,
            },
            ..Default::default()
        }
    }

    pub fn dock(theme: &Theme) -> container::Style {
        let (_, secondary_bg, border_color, _) = get_theme_colors(theme);
        
        container::Style {
            background: Some(secondary_bg.into()),
            border: Border {
                width: 1.0,
                color: border_color,
                radius: (16.0).into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.08),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 8.0,
            },
            ..Default::default()
        }
    }

    pub fn main_container(theme: &Theme) -> container::Style {
        let (bg_color, _, border_color, _) = get_theme_colors(theme);
        
        container::Style {
            background: Some(bg_color.into()),
            border: Border {
                width: 1.0,
                color: border_color,
                radius: (10.0).into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.05),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 10.0,
            },
            ..Default::default()
        }
    }

    pub fn header_text(theme: &Theme) -> iced::widget::text::Style {
        let (_, _, _, text_color) = get_theme_colors(theme);
        
        iced::widget::text::Style {
            color: Some(text_color),
            ..Default::default()
        }
    }

    pub fn panel_content(theme: &Theme) -> container::Style {
        let (bg_color, _, border_color, _) = get_theme_colors(theme);
        
        container::Style {
            background: Some(bg_color.into()),
            border: Border {
                width: 1.0,
                color: border_color,
                radius: (8.0).into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.03),
                offset: iced::Vector::new(0.0, 1.0),
                blur_radius: 3.0,
            },
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortOrder {
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