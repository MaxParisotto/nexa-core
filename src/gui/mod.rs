use iced::widget::{container, row, text, Column, Button};
use iced::{Element, Length, Theme, Color, Subscription, Command};
use iced::executor;
use iced::window;
use iced::{Application, Settings};
use std::sync::Arc;
use std::time::Duration;
use crate::server::{Server, ServerMetrics};
use crate::error::NexaError;
use crate::cli::CliHandler;
use log::{info, error};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    UpdateState(String, usize, ServerMetrics),
    StartServer,
    StopServer,
    ServerStarted(bool, Option<String>),
    ServerStopped(bool, Option<String>),
    Exit,
}

pub struct NexaApp {
    handler: Arc<CliHandler>,
    server_status: String,
    total_connections: u64,
    active_connections: u32,
    failed_connections: u64,
    last_error: Option<String>,
    uptime: Duration,
    should_exit: bool,
}

impl NexaApp {
    pub fn new(handler: Arc<CliHandler>) -> Self {
        info!("Creating new NexaApp instance");
        Self {
            handler,
            server_status: "Stopped".to_string(),
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
                row![
                    if self.server_status == "Running" {
                        Button::new("Stop Server")
                            .on_press(Message::StopServer)
                    } else {
                        Button::new("Start Server")
                            .on_press(Message::StartServer)
                    },
                    Button::new("Exit")
                        .on_press(Message::Exit)
                ].spacing(20)
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
    type Flags = Arc<CliHandler>;

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
                    if app.server_status == "Running" {
                        let handler = app.handler.clone();
                        Command::perform(async move {
                            let server = handler.get_server();
                            let state = format!("{:?}", server.get_state().await);
                            let active = server.get_active_connections().await;
                            let metrics = server.get_metrics().await;
                            (state, active, metrics)
                        }, |(state, active, metrics)| Message::UpdateState(state, active, metrics))
                    } else {
                        Command::none()
                    }
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
                        app.last_error = None;
                    } else {
                        app.server_status = "Error".to_string();
                        app.last_error = error.map(|e| format!("Failed to start server: {}", e));
                    }
                    Command::none()
                }
                Message::ServerStopped(success, error) => {
                    if success {
                        app.server_status = "Stopped".to_string();
                        app.last_error = None;
                        app.total_connections = 0;
                        app.active_connections = 0;
                        app.failed_connections = 0;
                    } else {
                        app.server_status = "Error".to_string();
                        app.last_error = error.map(|e| format!("Failed to stop server: {}", e));
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

pub fn run_gui(handler: Arc<CliHandler>) -> Result<(), NexaError> {
    info!("Starting Nexa GUI...");
    let settings = Settings::with_flags(handler);

    // Run the GUI on the main thread
    tokio::task::block_in_place(|| {
        NexaGui::run(settings)
            .map_err(|e| NexaError::System(format!("Failed to run GUI: {}", e)))
    })
} 