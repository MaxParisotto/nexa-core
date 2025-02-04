use iced::{
    widget::{
        button, column, container, text, row,
    },
    Element, Length, Theme, Subscription,
    executor, window, Application, Command,
};
use std::sync::Arc;
use std::time::Duration;

use crate::cli::CliHandler;

use super::types::{Message, NexaApp, View};
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

fn view_agents(_app: &NexaApp) -> Element<Message> {
    column![
        header("Agents"),
        section_container(
            text("Agent management interface coming soon...")
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