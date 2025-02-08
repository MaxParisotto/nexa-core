use iced::keyboard;
use log::debug;
use iced::widget::pane_grid::{self};
use iced::widget::{
    button, column, container, row, scrollable, text, Text, Row,
    text_input,
};
use iced::{Element, Length, Size, Subscription, Task};
use crate::models::agent::{Agent, AgentStatus, Task as AgentTask};
use crate::cli::{self, AgentConfig, AgentWorkflow as Workflow, WorkflowStep, WorkflowStatus, LLMModel};
use iced::Theme;
use std::time::Duration;
use iced::time;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::gui::components::{
    agents, workflows, tasks, settings, logs, styles,
};
use crate::gui::components::agents::AgentConfigState;
use crate::gui::components::settings::LLMServerConfig;

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
    Workflows,
    Tasks,
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
    workflows: Vec<Workflow>,
    selected_workflow: Option<String>,
    new_workflow_name: String,
    new_workflow_steps: Vec<WorkflowStep>,
    tasks: Vec<AgentTask>,
    selected_task: Option<String>,
    new_task_title: String,
    new_task_description: String,
    new_task_priority: i32,
    new_task_agent: Option<String>,
    new_task_deadline: Option<DateTime<Utc>>,
    new_task_estimated_duration: i64,
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
        let (panes, _) = pane_grid::State::new(Pane::new(0));

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
                workflows: Vec::new(),
                selected_workflow: None,
                new_workflow_name: String::new(),
                new_workflow_steps: Vec::new(),
                tasks: Vec::new(),
                selected_task: None,
                new_task_title: String::new(),
                new_task_description: String::new(),
                new_task_priority: 50,
                new_task_agent: None,
                new_task_deadline: None,
                new_task_estimated_duration: 3600,
            },
            Task::none(),
        )
    }

    fn title(&self) -> &'static str {
        "Nexa Agent Management"
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AgentMessage(_msg) => {
                // Handle agent messages
                Task::none()
            }
            Message::WorkflowMessage(_msg) => {
                // Handle workflow messages
                Task::none()
            }
            Message::TaskMessage(_msg) => {
                // Handle task messages
                Task::none()
            }
            Message::SettingsMessage(_msg) => {
                // Handle settings messages
                Task::none()
            }
            Message::LogMessage(_msg) => {
                // Handle log messages
                Task::none()
            }
            Message::ChangeView(view) => {
                self.current_view = view;
                Task::none()
            }
            Message::Tick => {
                // Handle periodic updates
                Task::none()
            }
            Message::Batch(messages) => {
                // Handle batch messages
                for message in messages {
                    self.update(message);
                }
                Task::none()
            }
            // Layout messages
            Message::Split(_axis, _pane) => {
                // Handle split
                Task::none()
            }
            Message::SplitFocused(_axis) => {
                // Handle split focused
                Task::none()
            }
            Message::FocusAdjacent(_direction) => {
                // Handle focus adjacent
                Task::none()
            }
            Message::Clicked(_pane) => {
                // Handle click
                Task::none()
            }
            Message::Dragged(_event) => {
                // Handle drag
                Task::none()
            }
            Message::Resized(_event) => {
                // Handle resize
                Task::none()
            }
            Message::TogglePin(_pane) => {
                // Handle toggle pin
                Task::none()
            }
            Message::Maximize(_pane) => {
                // Handle maximize
                Task::none()
            }
            Message::Restore => {
                // Handle restore
                Task::none()
            }
            Message::Close(_pane) => {
                // Handle close
                Task::none()
            }
            Message::CloseFocused => {
                // Handle close focused
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
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

        let dock = self.view_dock();

        container(
            column![
                content,
                dock,
            ]
            .spacing(20)
        )
        .padding(20)
        .style(styles::main_container)
        .into()
    }

    fn view_dock(&self) -> Element<Message> {
        let dock_items = self.dock_items.iter().map(|item| {
            container(
                column![
                    Text::new(&item.icon).size(24),
                    Text::new(&item.name).size(12)
                ]
                .spacing(5)
                .width(Length::Fill)
            )
            .padding(10)
            .style(styles::dock_item)
            .into()
        }).collect::<Vec<_>>();

        container(
            Row::with_children(dock_items)
                .spacing(10)
                .padding(10)
        )
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
    icon: String,
    action: Message,
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

impl Default for Example {
    fn default() -> Self {
        Example::new().0
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

    // Component-specific styles
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