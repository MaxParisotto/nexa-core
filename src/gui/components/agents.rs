use iced::widget::{column, container, row, text_input, Text};
use iced::Element;
use crate::models::agent::Agent;
use crate::cli::AgentConfig;
use crate::gui::components::{common, styles};

#[derive(Debug, Clone)]
pub enum AgentMessage {
    Start(String),
    Stop(String),
    ViewDetails(String),
    UpdateConfig(String, AgentConfig),
    UpdateMaxTasks(String),
    UpdatePriority(String),
    UpdateProvider(String),
    UpdateModel(String),
    UpdateTimeout(String),
    Back,
    Refresh,
}

pub fn view_agent_header<'a>() -> Element<'a, AgentMessage> {
    common::header("AI Agents Management")
}

pub fn view_agents_list<'a>(agents: &'a [Agent]) -> Element<'a, AgentMessage> {
    let agents_list = column(
        agents.iter().map(|agent| {
            view_agent_item(agent)
        }).collect::<Vec<_>>()
    )
    .spacing(10);

    common::section("Agents", agents_list.into())
}

fn view_agent_item<'a>(agent: &'a Agent) -> Element<'a, AgentMessage> {
    container(
        row![
            container(
                Text::new("●")
                    .size(12)
            )
            .style(styles::status_badge_style(agent.status)),
            Text::new(&agent.name)
                .size(16),
            Text::new(format!("Status: {:?}", agent.status))
                .size(14),
            if let Some(task) = &agent.current_task {
                Text::new(format!("Current Task: {}", task))
            } else {
                Text::new("No Active Task")
            }
            .size(14)
        ]
        .spacing(15)
        .align_y(iced::alignment::Vertical::Center)
    )
    .padding(10)
    .style(styles::panel_content)
    .into()
}

pub fn view_agent_details<'a>(
    agent: &'a Agent,
    config: &'a AgentConfigState,
) -> Element<'a, AgentMessage> {
    let header = container(
        Text::new(format!("Agent Details: {}", agent.name))
            .size(32)
            .style(styles::header_text)
    )
    .padding(20)
    .style(styles::panel_content);

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
                Text::new("Last Active: ").size(16),
                Text::new(agent.last_heartbeat.to_string()).size(16)
            ]
        ]
        .spacing(10)
    )
    .padding(20)
    .style(styles::panel_content);

    let config_form = container(
        column![
            Text::new("Configuration")
                .size(24)
                .style(styles::header_text),
            row![
                Text::new("Max Tasks: ").size(16),
                text_input(
                    "4",
                    &config.max_concurrent_tasks
                )
                .on_input(AgentMessage::UpdateMaxTasks)
                .padding(10)
            ],
            row![
                Text::new("Priority: ").size(16),
                text_input(
                    "50",
                    &config.priority_threshold
                )
                .on_input(AgentMessage::UpdatePriority)
                .padding(10)
            ]
        ]
        .spacing(10)
    )
    .padding(20)
    .style(styles::panel_content);

    column![
        header,
        details,
        config_form
    ]
    .spacing(20)
    .into()
}

#[derive(Debug, Clone)]
pub struct AgentConfigState {
    pub max_concurrent_tasks: String,
    pub priority_threshold: String,
    pub llm_provider: String,
    pub llm_model: String,
    pub timeout_seconds: String,
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