use iced::{
    widget::{
        button, column, container, text, row,
        text_input,
    },
    Element, Length, Theme, Subscription,
    executor, window, Application, Command,
    theme, Alignment, Color,
};
use std::sync::Arc;
use std::time::Duration;

use crate::cli::{CliHandler, AgentStatus, AgentConfig, RetryPolicy};

use super::types::{Message, NexaApp, View, AgentFormState};
use super::components::{header, sidebar_container, error_container, section_container};
use super::utils::format_duration;

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
        (Self { app: Some(app) }, Command::none())
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
                    app.uptime += Duration::from_secs(1);
                    Command::none()
                }
                Message::UpdateState(state, active_connections) => {
                    app.server_status = state;
                    app.active_connections = active_connections as u32;
                    Command::none()
                }
                Message::StartServer => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        match handler.start(None).await {
                            Ok(_) => (true, None),
                            Err(e) => (false, Some(e.to_string())),
                        }
                    }, |(success, error)| Message::ServerStarted(success, error))
                }
                Message::StopServer => {
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
                        app.server_logs.push("Server started successfully".to_string());
                    } else if let Some(err) = error {
                        app.server_status = "Error".to_string();
                        app.error_logs.push(format!("Failed to start server: {}", err));
                    }
                    Command::none()
                }
                Message::ServerStopped(success, error) => {
                    if success {
                        app.server_status = "Stopped".to_string();
                        app.server_logs.push("Server stopped successfully".to_string());
                    } else if let Some(err) = error {
                        app.server_status = "Error".to_string();
                        app.error_logs.push(format!("Failed to stop server: {}", err));
                    }
                    Command::none()
                }
                Message::Exit => {
                    app.should_exit = true;
                    window::close(window::Id::MAIN)
                }
                Message::ChangeView(view) => {
                    app.current_view = view;
                    Command::none()
                }
                Message::ShowAgentForm => {
                    app.agent_form.show_form = true;
                    Command::none()
                }
                Message::HideAgentForm => {
                    app.agent_form.show_form = false;
                    app.agent_form.validation_errors.clear();
                    Command::none()
                }
                Message::UpdateAgentName(name) => {
                    app.agent_form.name = name;
                    Command::none()
                }
                Message::UpdateAgentLLMProvider(provider) => {
                    app.agent_form.llm_provider = provider;
                    Command::none()
                }
                Message::UpdateAgentLLMModel(model) => {
                    app.agent_form.llm_model = model;
                    Command::none()
                }
                Message::UpdateAgentMaxTasks(tasks) => {
                    app.agent_form.max_concurrent_tasks = tasks;
                    Command::none()
                }
                Message::UpdateAgentPriority(priority) => {
                    app.agent_form.priority_threshold = priority;
                    Command::none()
                }
                Message::UpdateAgentTimeout(timeout) => {
                    app.agent_form.timeout_seconds = timeout;
                    Command::none()
                }
                Message::SubmitAgentForm => {
                    app.agent_form.validation_errors.clear();
                    let mut valid = true;

                    if app.agent_form.name.is_empty() {
                        app.agent_form.validation_errors.push("Agent name is required".to_string());
                        valid = false;
                    }
                    if app.agent_form.llm_provider.is_empty() {
                        app.agent_form.validation_errors.push("LLM provider is required".to_string());
                        valid = false;
                    }
                    if app.agent_form.llm_model.is_empty() {
                        app.agent_form.validation_errors.push("LLM model is required".to_string());
                        valid = false;
                    }

                    if let Err(_) = app.agent_form.max_concurrent_tasks.parse::<usize>() {
                        app.agent_form.validation_errors.push("Max concurrent tasks must be a positive number".to_string());
                        valid = false;
                    }
                    if let Err(_) = app.agent_form.priority_threshold.parse::<i32>() {
                        app.agent_form.validation_errors.push("Priority threshold must be a number".to_string());
                        valid = false;
                    }
                    if let Err(_) = app.agent_form.timeout_seconds.parse::<u64>() {
                        app.agent_form.validation_errors.push("Timeout must be a positive number".to_string());
                        valid = false;
                    }

                    if valid {
                        let config = AgentConfig {
                            max_concurrent_tasks: app.agent_form.max_concurrent_tasks.parse().unwrap(),
                            priority_threshold: app.agent_form.priority_threshold.parse().unwrap(),
                            llm_provider: app.agent_form.llm_provider.clone(),
                            llm_model: app.agent_form.llm_model.clone(),
                            retry_policy: RetryPolicy {
                                max_retries: 3,
                                backoff_ms: 1000,
                                max_backoff_ms: 10000,
                            },
                            timeout_seconds: app.agent_form.timeout_seconds.parse().unwrap(),
                        };

                        let name = app.agent_form.name.clone();
                        app.agent_form = AgentFormState::default();
                        Command::perform(async {}, |_| Message::CreateAgent(name, config))
                    } else {
                        Command::none()
                    }
                }
                Message::CreateAgent(name, config) => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.create_agent(name, config).await
                    }, Message::AgentCreated)
                }
                Message::AgentCreated(result) => {
                    match result {
                        Ok(agent) => {
                            app.agents.push(agent);
                            app.server_logs.push("Agent created successfully".to_string());
                        }
                        Err(e) => {
                            app.error_logs.push(format!("Failed to create agent: {}", e));
                        }
                    }
                    Command::none()
                }
                Message::UpdateAgentCapabilities(agent_id, capabilities) => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.update_agent_capabilities(&agent_id, capabilities).await
                    }, Message::CapabilitiesUpdated)
                }
                Message::CapabilitiesUpdated(result) => {
                    match result {
                        Ok(_) => {
                            app.server_logs.push("Agent capabilities updated successfully".to_string());
                            Command::perform(async {}, |_| Message::RefreshAgents)
                        }
                        Err(e) => {
                            app.error_logs.push(format!("Failed to update agent capabilities: {}", e));
                            Command::none()
                        }
                    }
                }
                Message::SetAgentHierarchy(parent_id, child_id) => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.set_agent_hierarchy(&parent_id, &child_id).await
                    }, Message::HierarchyUpdated)
                }
                Message::HierarchyUpdated(result) => {
                    match result {
                        Ok(_) => {
                            app.server_logs.push("Agent hierarchy updated successfully".to_string());
                            Command::perform(async {}, |_| Message::RefreshAgents)
                        }
                        Err(e) => {
                            app.error_logs.push(format!("Failed to update agent hierarchy: {}", e));
                            Command::none()
                        }
                    }
                }
                Message::RefreshAgents => {
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        handler.list_agents(None).await
                    }, Message::AgentsRefreshed)
                }
                Message::AgentsRefreshed(result) => {
                    match result {
                        Ok(agents) => {
                            app.agents = agents;
                            app.server_logs.push("Agents refreshed successfully".to_string());
                        }
                        Err(e) => {
                            app.error_logs.push(format!("Failed to refresh agents: {}", e));
                        }
                    }
                    Command::none()
                }
            }
        } else {
            Command::none()
        }
    }

    fn view(&self) -> Element<Message> {
        if let Some(app) = &self.app {
            let content = match app.current_view {
                View::Overview => view_overview(app),
                View::Agents => view_agents(app),
                View::Tasks => view_tasks(app),
                View::Connections => view_connections(app),
                View::Settings => view_settings(app),
                View::LLMServers => view_llm_servers(app),
            };

            row![
                sidebar_container(view_sidebar(app)),
                container(content)
                    .width(Length::Fill)
                    .padding(20)
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            text("Initializing...").into()
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

fn view_sidebar(_app: &NexaApp) -> Element<Message> {
    column![
        text("Menu").size(28),
        button(text("Overview"))
            .on_press(Message::ChangeView(View::Overview))
            .width(Length::Fill),
        button(text("Agents"))
            .on_press(Message::ChangeView(View::Agents))
            .width(Length::Fill),
        button(text("Tasks"))
            .on_press(Message::ChangeView(View::Tasks))
            .width(Length::Fill),
        button(text("Connections"))
            .on_press(Message::ChangeView(View::Connections))
            .width(Length::Fill),
        button(text("Settings"))
            .on_press(Message::ChangeView(View::Settings))
            .width(Length::Fill),
        button(text("LLM Servers"))
            .on_press(Message::ChangeView(View::LLMServers))
            .width(Length::Fill)
    ]
    .spacing(20)
    .padding(20)
    .into()
}

fn view_overview(app: &NexaApp) -> Element<Message> {
    let status_button = if app.server_status == "Running" {
        button(text("Stop Server"))
            .on_press(Message::StopServer)
    } else {
        button(text("Start Server"))
            .on_press(Message::StartServer)
    };

    let error_logs = if !app.error_logs.is_empty() {
        column(
            app.error_logs.iter()
                .map(|log| text(log).into())
                .collect::<Vec<Element<Message>>>()
        ).spacing(5)
    } else {
        column(vec![]).spacing(5)
    };

    let error_section: Element<Message> = if !app.error_logs.is_empty() {
        error_container(
            column![
                text("Error Logs").size(20),
                error_logs
            ].spacing(10)
        ).into()
    } else {
        container(text("")).into()
    };

    column![
        header("Overview"),
        section_container(
            column![
                text(format!("Server Status: {}", app.server_status)).size(20),
                text(format!("Uptime: {}", format_duration(app.uptime))).size(16),
                text(format!("Active Connections: {}", app.active_connections)).size(16),
                text(format!("Total Connections: {}", app.total_connections)).size(16),
                text(format!("Failed Connections: {}", app.failed_connections)).size(16),
                status_button
            ].spacing(10)
        ),
        section_container(
            column![
                text("Server Logs").size(20),
                column(
                    app.server_logs.iter()
                        .map(|log| text(log).into())
                        .collect::<Vec<Element<Message>>>()
                ).spacing(5)
            ].spacing(10)
        ),
        error_section
    ]
    .spacing(20)
    .into()
}

fn view_agents(app: &NexaApp) -> Element<Message> {
    let status_colors = |status: &AgentStatus| {
        match status {
            AgentStatus::Active => Color::from_rgb(0.0, 0.8, 0.0),
            AgentStatus::Idle => Color::from_rgb(0.8, 0.8, 0.0),
            AgentStatus::Busy => Color::from_rgb(0.8, 0.4, 0.0),
            AgentStatus::Error => Color::from_rgb(0.8, 0.0, 0.0),
            AgentStatus::Maintenance => Color::from_rgb(0.4, 0.4, 0.8),
            AgentStatus::Offline => Color::from_rgb(0.5, 0.5, 0.5),
        }
    };

    let form = if app.agent_form.show_form {
        section_container(
            column![
                row![
                    text("Create New Agent").size(24),
                    button(text("×").size(24))
                        .on_press(Message::HideAgentForm)
                        .style(theme::Button::Destructive)
                ]
                .spacing(20)
                .align_items(Alignment::Center),
                text_input("Agent Name", &app.agent_form.name)
                    .on_input(Message::UpdateAgentName)
                    .padding(10),
                text_input("LLM Provider", &app.agent_form.llm_provider)
                    .on_input(Message::UpdateAgentLLMProvider)
                    .padding(10),
                text_input("LLM Model", &app.agent_form.llm_model)
                    .on_input(Message::UpdateAgentLLMModel)
                    .padding(10),
                text_input("Max Concurrent Tasks", &app.agent_form.max_concurrent_tasks)
                    .on_input(Message::UpdateAgentMaxTasks)
                    .padding(10),
                text_input("Priority Threshold", &app.agent_form.priority_threshold)
                    .on_input(Message::UpdateAgentPriority)
                    .padding(10),
                text_input("Timeout (seconds)", &app.agent_form.timeout_seconds)
                    .on_input(Message::UpdateAgentTimeout)
                    .padding(10),
                if !app.agent_form.validation_errors.is_empty() {
                    column(
                        app.agent_form.validation_errors.iter()
                            .map(|error| text(error).style(theme::Text::Color(Color::from_rgb(0.8, 0.0, 0.0))))
                            .map(Element::from)
                            .collect::<Vec<Element<Message>>>()
                    ).spacing(5)
                } else {
                    column(vec![]).spacing(5)
                },
                button(text("Create Agent"))
                    .on_press(Message::SubmitAgentForm)
                    .style(theme::Button::Primary)
                    .width(Length::Fill)
            ]
            .spacing(15)
        )
    } else {
        container(
            button(text("+ Create New Agent"))
                .on_press(Message::ShowAgentForm)
                .style(theme::Button::Primary)
        ).into()
    };

    let content: Element<Message> = if !app.agents.is_empty() {
        column(
            app.agents.iter().map(|agent| {
                section_container(
                    column![
                        row![
                            column![
                                text(&agent.name).size(18),
                                text(&agent.id).size(12).style(Color::from_rgb(0.5, 0.5, 0.5))
                            ],
                            text(format!("{:?}", agent.status))
                                .style(status_colors(&agent.status))
                        ]
                        .spacing(10)
                        .align_items(Alignment::Center),
                        if !agent.capabilities.is_empty() {
                            text(format!("Capabilities: {}", agent.capabilities.join(", ")))
                                .size(14)
                        } else {
                            text("No capabilities").size(14)
                        },
                        row![
                            text(format!("Tasks completed: {}", agent.metrics.tasks_completed)),
                            text(format!("Tasks failed: {}", agent.metrics.tasks_failed)),
                            text(format!("Avg response: {:.2}ms", agent.metrics.average_response_time_ms))
                        ]
                        .spacing(20),
                        if let Some(error) = &agent.metrics.last_error {
                            text(format!("Last error: {}", error))
                                .style(Color::from_rgb(0.8, 0.0, 0.0))
                        } else {
                            text("")
                        },
                        row![
                            button(text("Update Capabilities"))
                                .on_press(Message::UpdateAgentCapabilities(
                                    agent.id.clone(),
                                    vec!["code_generation".to_string(), "code_review".to_string()]
                                )),
                            if agent.parent_id.is_none() {
                                button(text("Set as Child"))
                                    .on_press(Message::SetAgentHierarchy(
                                        "parent_id".to_string(),
                                        agent.id.clone()
                                    ))
                            } else {
                                button(text("Remove Parent"))
                                    .on_press(Message::SetAgentHierarchy(
                                        agent.id.clone(),
                                        "".to_string()
                                    ))
                            }
                        ]
                        .spacing(10)
                    ]
                    .spacing(10)
                ).into()
            })
            .collect::<Vec<Element<Message>>>()
        ).spacing(10).into()
    } else {
        text("No agents found. Create one to get started.")
            .size(16)
            .style(Color::from_rgb(0.5, 0.5, 0.5))
            .into()
    };

    column![
        header("Agents"),
        section_container(
            column![
                row![
                    text("Agents").size(20),
                    button(text("Refresh"))
                        .on_press(Message::RefreshAgents)
                        .style(theme::Button::Secondary)
                ]
                .spacing(20)
                .align_items(Alignment::Center),
                form,
                content
            ]
            .spacing(20)
        )
    ]
    .spacing(20)
    .into()
}

fn view_tasks(_app: &NexaApp) -> Element<Message> {
    column![
        header("Tasks"),
        section_container(
            text("Task management interface coming soon...")
        )
    ]
    .spacing(20)
    .into()
}

fn view_connections(_app: &NexaApp) -> Element<Message> {
    column![
        header("Connections"),
        section_container(
            text("Connection management interface coming soon...")
        )
    ]
    .spacing(20)
    .into()
}

fn view_settings(_app: &NexaApp) -> Element<Message> {
    column![
        header("Settings"),
        section_container(
            text("Settings interface coming soon...")
        )
    ]
    .spacing(20)
    .into()
}

fn view_llm_servers(_app: &NexaApp) -> Element<Message> {
    column![
        header("LLM Servers"),
        section_container(
            text("LLM server management interface coming soon...")
        )
    ]
    .spacing(20)
    .into()
} 