use iced::widget::{container, row, text, Column, Button};
use iced::{Element, Length, Theme, Color, Subscription, Command};
use iced::executor;
use iced::window;
use iced::{Application, Settings};
use std::sync::Arc;
use std::time::Duration;
use crate::server::{Server, ServerMetrics};
use crate::error::NexaError;
use log::info;

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    UpdateState(String, usize, ServerMetrics),
    Exit,
}

pub struct NexaApp {
    server: Arc<Server>,
    server_status: String,
    total_connections: u64,
    active_connections: u32,
    failed_connections: u64,
    last_error: Option<String>,
    uptime: Duration,
    should_exit: bool,
}

impl NexaApp {
    pub fn new(server: Arc<Server>) -> Self {
        info!("Creating new NexaApp instance");
        Self {
            server,
            server_status: "Initializing...".to_string(),
            total_connections: 0,
            active_connections: 0,
            failed_connections: 0,
            last_error: None,
            uptime: Duration::from_secs(0),
            should_exit: false,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let status_color = match self.server_status.as_str() {
            "Running" => Color::from_rgb(0.0, 0.8, 0.0),
            "Stopped" => Color::from_rgb(0.8, 0.0, 0.0),
            _ => Color::from_rgb(0.8, 0.8, 0.0),
        };

        let content = Column::new()
            .spacing(20)
            .padding(20)
            .push(
                text(&format!("Status: {}", self.server_status))
                    .style(status_color)
            )
            .push(
                row![
                    text("Total Connections: "),
                    text(self.total_connections.to_string())
                ]
            )
            .push(
                row![
                    text("Active Connections: "),
                    text(self.active_connections.to_string())
                ]
            )
            .push(
                row![
                    text("Failed Connections: "),
                    text(self.failed_connections.to_string())
                ]
            )
            .push(if let Some(error) = &self.last_error {
                row![
                    text("Last Error: "),
                    text(error).style(Color::from_rgb(0.8, 0.0, 0.0))
                ]
            } else {
                row![]
            })
            .push(
                text(format!("Uptime: {:?}", self.uptime))
            )
            .push(
                Button::new("Exit")
                    .on_press(Message::Exit)
            );

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

#[derive(Default)]
pub struct NexaGui {
    app: Option<NexaApp>,
}

impl Application for NexaGui {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = Arc<Server>;

    fn new(flags: Self::Flags) -> (Self, Command<Message>) {
        let app = NexaApp::new(flags);
        
        (
            Self { app: Some(app) },
            Command::none(),
        )
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
                    if app.should_exit {
                        return window::close();
                    }
                    app.uptime += Duration::from_secs(1);
                    let server = app.server.clone();
                    Command::perform(async move {
                        let state = format!("{:?}", server.get_state().await);
                        let active = server.get_active_connections().await;
                        let metrics = server.get_metrics().await;
                        (state, active, metrics)
                    }, |(state, active, metrics)| Message::UpdateState(state, active, metrics))
                }
                Message::UpdateState(state, active, metrics) => {
                    app.server_status = state;
                    app.active_connections = active as u32;
                    app.total_connections = metrics.total_connections;
                    app.failed_connections = metrics.failed_connections;
                    if let Some(error) = metrics.last_error {
                        app.last_error = Some(error);
                    }
                    Command::none()
                }
                Message::Exit => {
                    app.should_exit = true;
                    Command::none()
                }
            }
        } else {
            Command::none()
        }
    }

    fn view(&self) -> Element<Message> {
        if let Some(app) = &self.app {
            app.view()
        } else {
            text("Initializing...").into()
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

pub fn run_gui(server: Arc<Server>) -> Result<(), NexaError> {
    info!("Starting Nexa GUI...");
    let settings = Settings::with_flags(server);

    NexaGui::run(settings)
        .map_err(|e| NexaError::System(format!("Failed to run GUI: {}", e)))
} 