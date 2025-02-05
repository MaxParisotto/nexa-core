use iced::{
    widget::{ button, container, text, row, column, text_input, Button, Container, Text, Row, Column, TextInput },
    Element, Length, Theme, Subscription,
    executor, Alignment, Color, alignment,
    advanced::command::Command,
    advanced::program::Program,
};

use std::sync::Arc;
use std::time::Duration;
use std::marker::PhantomData;

use crate::cli::{CliHandler, AgentStatus, AgentConfig, RetryPolicy, Agent};
use super::types::{Message, NexaApp, View, AgentFormState};
use super::styles;
use super::components::{header, error_container, section_container};
use super::utils::format_duration;

/// Main application struct
pub struct NexaGui {
    app: Option<NexaApp>,
}

impl Program for NexaGui {
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

            container(
                Column::with_children(vec![
                    view_sidebar(app),
                    container(content)
                        .width(Length::Fill)
                        .padding(20)
                        .style(styles::content_container())
                        .into()
                ])
                .width(Length::Fill)
                .height(Length::Fill)
            )
            .style(styles::main_container())
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            container(
                text("Initializing...")
                    .size(24)
                    .style(Color::from_rgb(0.7, 0.7, 0.7))
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center)
            .into()
        }
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
                    Command::none()
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
}

fn view_sidebar(app: &NexaApp) -> iced::Element<Message> {
    let menu_button = |label: &str, view: View| -> iced::Element<Message> {
        button(
            text(label)
                .size(16)
                .width(Length::Fill)
        )
        .width(Length::Fill)
        .padding(10)
        .style(if app.current_view == view {
            styles::primary_button()
        } else {
            styles::secondary_button()
        })
        .on_press(Message::ChangeView(view))
        .into()
    };

    let buttons: Vec<iced::Element<Message>> = vec![
        menu_button("Overview", View::Overview),
        menu_button("Agents", View::Agents),
        menu_button("Tasks", View::Tasks),
        menu_button("Connections", View::Connections),
        menu_button("Settings", View::Settings),
        menu_button("LLM Servers", View::LLMServers),
    ];

    container(
        Column::with_children(vec![
            container(
                text("NEXA CORE")
                    .size(24)
                    .style(Color::from_rgb(0.9, 0.9, 0.9))
            )
            .padding(20)
            .width(Length::Fill)
            .center_x()
            .align_y(alignment::Vertical::Center)
            .into(),
            
            Column::with_children(buttons)
                .spacing(5)
                .padding(10)
                .into(),

            container(
                button(
                    text("Exit")
                        .size(16)
                        .width(Length::Fill)
                )
                .width(Length::Fill)
                .padding(10)
                .style(styles::secondary_button())
                .on_press(Message::Exit)
            )
            .padding(10)
            .into()
        ])
        .height(Length::Fill)
    )
    .width(Length::Fixed(200.0))
    .height(Length::Fill)
    .style(styles::container_style(styles::SidebarContainer))
    .into()
}

fn view_overview(app: &NexaApp) -> iced::Element<Message> {
    let status_button = if app.server_status == "Running" {
        button(text("Stop Server"))
            .on_press(Message::StopServer)
            .into()
    } else {
        button(text("Start Server"))
            .on_press(Message::StartServer)
            .into()
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
            Column::with_children(vec![
                text("Error Logs").size(20).into(),
                error_logs
            ]).spacing(10)
        ).into()
    } else {
        container(text("")).into()
    };

    Column::with_children(vec![
        header("Overview").into(),
        section_container(
            Column::with_children(vec![
                text(format!("Server Status: {}", app.server_status)).size(20),
                text(format!("Uptime: {}", format_duration(app.uptime))).size(16),
                text(format!("Active Connections: {}", app.active_connections)).size(16),
                text(format!("Total Connections: {}", app.total_connections)).size(16),
                text(format!("Failed Connections: {}", app.failed_connections)).size(16),
                status_button
            ])
            .spacing(10)
            .into()
        ),
        section_container(
            Column::with_children(vec![
                text("Server Logs").size(20),
                column(
                    app.server_logs.iter()
                        .map(|log| text(log).into())
                        .collect::<Vec<Element<Message>>>()
                ).spacing(5)
            ])
            .spacing(10)
        ),
        error_section.into()
    ])
    .spacing(20)
    .into()
}

fn view_agents(app: &NexaApp) -> iced::Element<Message> {
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
            Column::with_children(vec![
                Row::with_children(vec![
                    text("Create New Agent").size(24).into(),
                    button(text("×").size(24))
                        .on_press(Message::HideAgentForm)
                        .style(Button::Destructive)
                        .into()
                ]).spacing(20).align_items(Alignment::Center),
                { let mut name_state = TextInput::State::default();
                  TextInput::new(&mut name_state, "Agent Name", |s| Message::UpdateAgentName(s))
                      .value(&app.agent_form.name)
                      .padding(10) 
                },
                { let mut provider_state = TextInput::State::default();
                  TextInput::new(&mut provider_state, "LLM Provider", |s| Message::UpdateAgentLLMProvider(s))
                      .value(&app.agent_form.llm_provider)
                      .padding(10) 
                },
                { let mut model_state = TextInput::State::default();
                  TextInput::new(&mut model_state, "LLM Model", |s| Message::UpdateAgentLLMModel(s))
                      .value(&app.agent_form.llm_model)
                      .padding(10) 
                },
                { let mut max_tasks_state = TextInput::State::default();
                  TextInput::new(&mut max_tasks_state, "Max Concurrent Tasks", |s| Message::UpdateAgentMaxTasks(s))
                      .value(&app.agent_form.max_concurrent_tasks)
                      .padding(10) 
                },
                { let mut priority_state = TextInput::State::default();
                  TextInput::new(&mut priority_state, "Priority Threshold", |s| Message::UpdateAgentPriority(s))
                      .value(&app.agent_form.priority_threshold)
                      .padding(10) 
                },
                { let mut timeout_state = TextInput::State::default();
                  TextInput::new(&mut timeout_state, "Timeout (seconds)", |s| Message::UpdateAgentTimeout(s))
                      .value(&app.agent_form.timeout_seconds)
                      .padding(10) 
                },
                if !app.agent_form.validation_errors.is_empty() {
                    column(
                        app.agent_form.validation_errors.iter()
                            .map(|error| text(error).style(Color::from_rgb(0.8, 0.0, 0.0)))
                            .map(Element::from)
                            .collect::<Vec<Element<Message>>>()
                    ).spacing(5)
                } else {
                    column(vec![]).spacing(5)
                },
                button(text("Create Agent"))
                    .on_press(Message::SubmitAgentForm)
                    .style(Button::Primary)
                    .width(Length::Fill)
            ])
            .spacing(15)
        )
    } else {
        container(
            button(text("+ Create New Agent"))
                .on_press(Message::ShowAgentForm)
                .style(Button::Primary)
        ).into()
    };

    let content: Element<Message> = if !app.agents.is_empty() {
        Column::with_children(
            app.agents.iter().map(|agent| {
                section_container(
                    Column::with_children(vec![
                        Row::with_children(vec![
                            Column::with_children(vec![
                                text(&agent.name).size(18).into(),
                                text(&agent.id).size(12).style(Color::from_rgb(0.5, 0.5, 0.5)).into()
                            ]),
                            Row::with_children(vec![
                                Column::with_children(vec![
                                    text(&agent.name).size(18).into(),
                                    text(&agent.id).size(12).style(Color::from_rgb(0.5, 0.5, 0.5)).into()
                                ]),
                                text(format!("{:?}", agent.status))
                                    .style(status_colors(&agent.status))
                                    .into()
                            ]).spacing(10).align_items(Alignment::Center)
                        ]).spacing(10),
                        if !agent.capabilities.is_empty() {
                            text(format!("Capabilities: {}", agent.capabilities.join(", "))).size(14)
                        } else {
                            text("No capabilities").size(14)
                        },
                        Row::with_children(vec![
                            text(format!("Tasks completed: {}", agent.metrics.tasks_completed)).into(),
                            text(format!("Tasks failed: {}", agent.metrics.tasks_failed)).into(),
                            text(format!("Avg response: {:.2}ms", agent.metrics.average_response_time_ms)).into()
                        ]).spacing(20),
                        if let Some(error) = &agent.metrics.last_error {
                            text(format!("Last error: {}", error)).style(Color::from_rgb(0.8, 0.0, 0.0))
                        } else {
                            text("")
                        },
                        Row::with_children(vec![
                            button(text("Update Capabilities"))
                                .on_press(Message::UpdateAgentCapabilities(
                                    agent.id.clone(),
                                    vec!["code_generation".to_string(), "code_review".to_string()]
                                ))
                                .into(),
                            if agent.parent_id.is_none() {
                                button(text("Set as Child"))
                                    .on_press(Message::SetAgentHierarchy(
                                        "parent_id".to_string(),
                                        agent.id.clone()
                                    ))
                                    .into()
                            } else {
                                button(text("Remove Parent"))
                                    .on_press(Message::SetAgentHierarchy(
                                        agent.id.clone(),
                                        "".to_string()
                                    ))
                                    .into()
                            }
                        ]).spacing(10)
                    ])
                ).into()
            }).collect::<Vec<Element<Message>>>()
        ).spacing(10).into()
    } else {
        text("No agents found. Create one to get started.")
            .size(16)
            .style(Color::from_rgb(0.5, 0.5, 0.5))
            .into()
    };

    Column::with_children(vec![
        header("Agents").into(),
        section_container(
            Column::with_children(vec![
                row![
                    text("Agents").size(20),
                    button(text("Refresh"))
                        .on_press(Message::RefreshAgents)
                        .style(Button::Secondary)
                ]
                .spacing(20)
                .align_items(Alignment::Center),
                form,
                content
            ])
            .spacing(20)
        )
    ])
    .spacing(20)
    .into()
}

fn view_tasks(_app: &NexaApp) -> Element<Message> {
    Column::with_children(vec![
        header("Tasks").into(),
        section_container(
            text("Task management interface coming soon...")
        ).into()
    ])
    .spacing(20)
    .into()
}

fn view_connections(_app: &NexaApp) -> Element<Message> {
    Column::with_children(vec![
        header("Connections").into(),
        section_container(
            text("Connection management interface coming soon...")
        ).into()
    ])
    .spacing(20)
    .into()
}

fn view_settings(_app: &NexaApp) -> Element<Message> {
    Column::with_children(vec![
        header("Settings").into(),
        section_container(
            text("Settings interface coming soon...")
        ).into()
    ])
    .spacing(20)
    .into()
}

fn view_llm_servers(_app: &NexaApp) -> Element<Message> {
    Column::with_children(vec![
        header("LLM Servers").into(),
        section_container(
            text("LLM server management interface coming soon...")
        ).into()
    ])
    .spacing(20)
    .into()
} 