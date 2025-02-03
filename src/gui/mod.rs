use iced::widget::{container, row, text, Column, Button, scrollable, Scrollable, Rule};
use iced::{Element, Length, Theme, Color, Subscription, Command};
use iced::executor;
use iced::window;
use iced::{Application, Settings};
use std::sync::Arc;
use std::time::Duration;
use std::collections::VecDeque;
use crate::server::{Server, ServerMetrics};
use crate::error::NexaError;
use crate::cli::CliHandler;
use log::{info, error};

const MAX_LOG_ENTRIES: usize = 100;

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

#[derive(Debug, Clone)]
struct LogEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    message: String,
    level: LogLevel,
}

#[derive(Debug, Clone, Copy)]
enum LogLevel {
    Info,
    Error,
    Debug,
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
    server_logs: VecDeque<LogEntry>,
    connection_logs: VecDeque<LogEntry>,
    error_logs: VecDeque<LogEntry>,
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
            server_logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            connection_logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            error_logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
        }
    }

    fn add_log(&mut self, message: String, level: LogLevel, log_type: &str) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            message,
            level,
        };

        let logs = match log_type {
            "server" => &mut self.server_logs,
            "connection" => &mut self.connection_logs,
            "error" => &mut self.error_logs,
            _ => return,
        };

        if logs.len() >= MAX_LOG_ENTRIES {
            logs.pop_front();
        }
        logs.push_back(entry);
    }

    fn view_log_section(&self, title: &str, logs: &VecDeque<LogEntry>) -> Element<Message> {
        let mut log_content = Column::new()
            .spacing(5)
            .push(text(title).size(20));

        for entry in logs.iter() {
            let color = match entry.level {
                LogLevel::Info => Color::from_rgb(0.0, 0.8, 0.0),
                LogLevel::Error => Color::from_rgb(0.8, 0.0, 0.0),
                LogLevel::Debug => Color::from_rgb(0.5, 0.5, 0.5),
            };

            log_content = log_content.push(
                text(format!("[{}] {}", 
                    entry.timestamp.format("%H:%M:%S"),
                    entry.message
                )).style(color).size(14)
            );
        }

        scrollable(
            container(log_content)
                .width(Length::Fill)
                .padding(10)
        )
        .height(Length::Fixed(200.0))
        .into()
    }

    pub fn view(&self) -> Element<Message> {
        let status_color = match self.server_status.as_str() {
            "Running" => Color::from_rgb(0.0, 0.8, 0.0),
            "Stopped" => Color::from_rgb(0.8, 0.0, 0.0),
            _ => Color::from_rgb(0.8, 0.8, 0.0),
        };

        // Status Section
        let status_section = Column::new()
            .spacing(10)
            .push(
                text("Server Status").size(24)
            )
            .push(
                text(&format!("Status: {}", self.server_status))
                    .style(status_color)
                    .size(20)
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

        // Metrics Section
        let metrics_section = Column::new()
            .spacing(10)
            .push(text("Server Metrics").size(24))
            .push(
                container(Column::new()
                    .spacing(5)
                    .push(text(format!("Uptime: {:?}", self.uptime)))
                    .push(text(format!("Total Connections: {}", self.total_connections)))
                    .push(text(format!("Active Connections: {}", self.active_connections)))
                    .push(text(format!("Failed Connections: {}", self.failed_connections)))
                )
                .padding(10)
            );

        // Error Section
        let error_section = if let Some(error) = &self.last_error {
            container(
                text(format!("Error: {}", error))
                    .style(Color::from_rgb(0.8, 0.0, 0.0))
            )
            .padding(10)
        } else {
            container(text("No errors"))
                .padding(10)
        };

        // Main Layout
        let content = Column::new()
            .spacing(20)
            .padding(20)
            .push(status_section)
            .push(Rule::horizontal(10))
            .push(metrics_section)
            .push(Rule::horizontal(10))
            .push(error_section)
            .push(Rule::horizontal(10))
            .push(
                row![
                    Column::new()
                        .width(Length::FillPortion(1))
                        .push(self.view_log_section("Server Logs", &self.server_logs)),
                    Column::new()
                        .width(Length::FillPortion(1))
                        .push(self.view_log_section("Connection Logs", &self.connection_logs)),
                    Column::new()
                        .width(Length::FillPortion(1))
                        .push(self.view_log_section("Error Logs", &self.error_logs)),
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
                        app.last_error = Some(error.clone());
                        app.add_log(error, LogLevel::Error, "error");
                    }
                    app.add_log(
                        format!("Active: {}, Total: {}, Failed: {}", 
                            active, 
                            metrics.total_connections,
                            metrics.failed_connections
                        ),
                        LogLevel::Debug,
                        "connection"
                    );
                    Command::none()
                }
                Message::StartServer => {
                    app.add_log("Starting server...".to_string(), LogLevel::Info, "server");
                    let handler = app.handler.clone();
                    Command::perform(async move {
                        match handler.start(None).await {
                            Ok(_) => (true, None),
                            Err(e) => (false, Some(e.to_string())),
                        }
                    }, |(success, error)| Message::ServerStarted(success, error))
                }
                Message::StopServer => {
                    app.add_log("Stopping server...".to_string(), LogLevel::Info, "server");
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
                        app.add_log("Server started successfully".to_string(), LogLevel::Info, "server");
                    } else {
                        app.server_status = "Error".to_string();
                        if let Some(err) = error {
                            app.last_error = Some(format!("Failed to start server: {}", err));
                            app.add_log(format!("Failed to start server: {}", err), LogLevel::Error, "error");
                        }
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
                        app.add_log("Server stopped successfully".to_string(), LogLevel::Info, "server");
                    } else {
                        app.server_status = "Error".to_string();
                        if let Some(err) = error {
                            app.last_error = Some(format!("Failed to stop server: {}", err));
                            app.add_log(format!("Failed to stop server: {}", err), LogLevel::Error, "error");
                        }
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