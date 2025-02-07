use iced::keyboard;
use log::debug;
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    button, column, container, row, scrollable, text, Text,
};  
use iced::{Color, Element, Fill, Length, Size, Subscription, Theme};
use crate::models::agent::{Agent, AgentStatus};

pub fn main() -> iced::Result {
    iced::application("Pane Grid - Iced", Example::update, Example::view)
        .subscription(Example::subscription)
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
}

impl Example {
    fn new() -> Self {
        let (panes, _) = pane_grid::State::new(Pane::new(0));

        Example {
            panes,
            panes_created: 1,
            focus: None,
            cli_handler: std::sync::Arc::new(crate::cli::CliHandler::new()),
            agents: Vec::new(),
            selected_agent: None,
            logs: Vec::new(),
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Split(axis, pane) => {
                let result =
                    self.panes.split(axis, pane, Pane::new(self.panes_created));

                if let Some((pane, _)) = result {
                    self.focus = Some(pane);
                }

                self.panes_created += 1;
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
            }
            Message::FocusAdjacent(direction) => {
                if let Some(pane) = self.focus {
                    if let Some(adjacent) = self.panes.adjacent(pane, direction)
                    {
                        self.focus = Some(adjacent);
                    }
                }
            }
            Message::Clicked(pane) => {
                self.focus = Some(pane);
            }
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            }
            Message::Dragged(pane_grid::DragEvent::Dropped {
                pane,
                target,
            }) => {
                self.panes.drop(pane, target);
            }
            Message::Dragged(_) => {}
            Message::TogglePin(pane) => {
                if let Some(Pane { is_pinned, .. }) = self.panes.get_mut(pane) {
                    *is_pinned = !*is_pinned;
                }
            }
            Message::Maximize(pane) => self.panes.maximize(pane),
            Message::Restore => {
                self.panes.restore();
            }
            Message::Close(pane) => {
                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focus = Some(sibling);
                }
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
            }
            Message::CliStart => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.start(None).await {
                        eprintln!("Failed to start server: {:?}", e);
                    }
                });
            }
            Message::CliStop => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.stop().await {
                        eprintln!("Failed to stop server: {:?}", e);
                    }
                });
            }
            Message::CliStatus => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.status().await {
                        eprintln!("Failed to get status: {:?}", e);
                    }
                });
            }
            Message::RefreshAgents => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Ok(agents) = handler.list_agents(None).await {
                        debug!("Found {} agents", agents.len());
                    }
                });
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
            }
            Message::StopAgent(id) => {
                println!("Stopping agent {}", id);
            }
            Message::ViewAgentDetails(id) => {
                self.selected_agent = Some(id);
            }
            Message::ConnectLLM(_id) => {
                // Implementation for connecting to an LLM
            }
            Message::DisconnectLLM(_id) => {
                // Implementation for disconnecting from an LLM
            }
            Message::AgentsUpdated(new_agents) => {
                self.agents = new_agents;
            }
            Message::UpdateAgentConfig(id, config) => {
                let handler = self.cli_handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = handler.update_agent_config(id, config).await {
                        eprintln!("Failed to update agent config: {}", e);
                    }
                });
            }
            Message::ShowLogs(_id) => {
                // Implementation for showing logs for a specific agent
            }
            Message::ClearLogs => {
                self.logs.clear();
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key_code, modifiers| {
            if !modifiers.command() {
                return None;
            }

            handle_hotkey(key_code)
        })
    }

    fn view(&self) -> Element<Message> {
        let left_panel = self.view_agent_panel();
        let right_panel = PaneGrid::new(&self.panes, |id, pane, is_maximized| {
            let is_focused = self.focus == Some(id);

            let pin_button = button(
                text(if pane.is_pinned { "Unpin" } else { "Pin" }).size(14),
            )
            .on_press(Message::TogglePin(id))
            .padding(3);

            let title = row![
                pin_button,
                "Pane",
                text(pane.id.to_string()).color(if is_focused {
                    PANE_ID_COLOR_FOCUSED
                } else {
                    PANE_ID_COLOR_UNFOCUSED
                }),
            ]
            .spacing(5);

            let title_bar = pane_grid::TitleBar::new(title)
                .controls(pane_grid::Controls::dynamic(
                    view_controls(
                        id,
                        self.panes.len(),
                        pane.is_pinned,
                        is_maximized,
                    ),
                    button(text("X").size(14))
                        .style(button::danger)
                        .padding(3)
                        .on_press_maybe(
                            if self.panes.len() > 1 && !pane.is_pinned {
                                Some(Message::Close(id))
                            } else {
                                None
                            },
                        ),
                ))
                .padding(10)
                .style(if is_focused {
                    style::title_bar_focused
                } else {
                    style::title_bar_active
                });

            pane_grid::Content::new(responsive(move |size| {
                view_content(id, self.panes.len(), pane.is_pinned, size)
            }))
            .title_bar(title_bar)
            .style(if is_focused {
                style::pane_focused
            } else {
                style::pane_active
            })
        })
        .width(Fill)
        .height(Fill)
        .spacing(10)
        .on_click(Message::Clicked)
        .on_drag(Message::Dragged)
        .on_resize(10, Message::Resized);

        let content = row![
            container(left_panel).width(Length::FillPortion(1)),
            container(right_panel).width(Length::FillPortion(2)),
        ]
        .spacing(20);

        container(content)
            .padding(10)
            .into()
    }

    #[allow(dead_code)]
    fn view_cli_panel(&self) -> Element<Message> {
        let start_btn = button(text("Start Server").size(16))
            .padding(8)
            .on_press(Message::CliStart);
        let stop_btn = button(text("Stop Server").size(16))
            .padding(8)
            .on_press(Message::CliStop);
        let status_btn = button(text("Server Status").size(16))
            .padding(8)
            .on_press(Message::CliStatus);
        let panel = column![start_btn, stop_btn, status_btn]
            .spacing(10)
            .align_x(iced::alignment::Horizontal::Center);
        container(panel)
            .padding(10)
            .center_x(Fill)
            .into()
    }

    fn view_agent_panel(&self) -> Element<Message> {
        let header = Text::new("AI Agents Management")
            .size(24)
            .style(move |_: &Theme| iced::widget::text::Style {
                color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                ..Default::default()
            });

        let logs_section = container(
            column![
                Text::new("System Logs").size(16),
                scrollable(
                    column(Vec::from_iter(
                        self.logs.iter()
                            .map(|log| Text::new(log.clone()).size(12).into())
                            .collect::<Vec<_>>()
                    ))
                ).height(Length::Fixed(100.0))
            ]
        ).padding(10);

        let mut agent_list = column![header].spacing(10);

        for agent in &self.agents {
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

        let refresh_button = button("Refresh")
            .on_press(Message::RefreshAgents)
            .padding(10);

        container(
            column![
                agent_list,
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
}

impl Default for Example {
    fn default() -> Self {
        Example::new()
    }
}

const PANE_ID_COLOR_UNFOCUSED: Color = Color::from_rgb(
    0xFF as f32 / 255.0,
    0xC7 as f32 / 255.0,
    0xC7 as f32 / 255.0,
);
const PANE_ID_COLOR_FOCUSED: Color = Color::from_rgb(
    0xFF as f32 / 255.0,
    0x47 as f32 / 255.0,
    0x47 as f32 / 255.0,
);

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

fn view_content<'a>(
    pane: pane_grid::Pane,
    total_panes: usize,
    is_pinned: bool,
    size: Size,
) -> Element<'a, Message> {
    // Helper to create a consistently styled button
    let button_builder = |label, message| {
        button(text(label).size(16))
            .width(Fill)
            .padding(12) // Increased padding for larger click area
            .on_press(message)
    };

    // Control buttons for splitting and (optionally) closing the pane
    let controls = column![
        button_builder(
            "Split horizontally",
            Message::Split(pane_grid::Axis::Horizontal, pane),
        ),
        button_builder(
            "Split vertically",
            Message::Split(pane_grid::Axis::Vertical, pane),
        )
    ]
    .push_maybe(if total_panes > 1 && !is_pinned {
        Some(button_builder("Close", Message::Close(pane)).style(button::danger))
    } else {
        None
    })
    .spacing(10)
    .max_width(180);

    // Pane content that shows the current size and the controls, with increased spacing and padding
    let content = column![
        text(format!("{} x {}", size.width, size.height))
            .size(28),
        controls
    ]
    .spacing(15)
    .align_x(iced::alignment::Horizontal::Center)
    .padding(15);

    // Wrap the content in a scrollable container with extra overall padding for visual breathing room
    container(scrollable(content))
        .center_y(Fill)
        .padding(20)
        .into()
}

fn view_controls<'a>(
    pane: pane_grid::Pane,
    total_panes: usize,
    is_pinned: bool,
    is_maximized: bool,
) -> Element<'a, Message> {
    let row = row![].spacing(5).push_maybe(if total_panes > 1 {
        let (content, message) = if is_maximized {
            ("Restore", Message::Restore)
        } else {
            ("Maximize", Message::Maximize(pane))
        };

        Some(
            button(text(content).size(14))
                .style(button::secondary)
                .padding(3)
                .on_press(message),
        )
    } else {
        None
    });

    let close = button(text("Close").size(14))
        .style(button::danger)
        .padding(3)
        .on_press_maybe(if total_panes > 1 && !is_pinned {
            Some(Message::Close(pane))
        } else {
            None
        });

    row.push(close).into()
}

fn responsive<'a, Message>(
    f: impl Fn(Size) -> Element<'a, Message> + 'a,
) -> Element<'a, Message> 
where
    Message: 'a,
{
    let content = f(Size::new(0.0, 0.0));
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

mod style {
    use iced::widget::container;
    use iced::{Border, Theme};

    pub fn title_bar_active(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            text_color: Some(palette.background.strong.text),
            background: Some(palette.background.strong.color.into()),
            ..Default::default()
        }
    }

    pub fn title_bar_focused(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            text_color: Some(palette.primary.strong.text),
            background: Some(palette.primary.strong.color.into()),
            ..Default::default()
        }
    }

    pub fn pane_active(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            background: Some(palette.background.weak.color.into()),
            border: Border {
                width: 2.0,
                color: palette.background.strong.color,
                ..Border::default()
            },
            ..Default::default()
        }
    }

    pub fn pane_focused(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            background: Some(palette.background.weak.color.into()),
            border: Border {
                width: 2.0,
                color: palette.primary.strong.color,
                ..Border::default()
            },
            ..Default::default()
        }
    }
} // end of style mod