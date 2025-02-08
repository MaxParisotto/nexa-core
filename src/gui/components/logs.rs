use iced::widget::{column, container, Text};
use iced::Element;
use super::{common, styles};
use crate::gui::app::Message;

#[derive(Debug, Clone)]
pub enum LogMessage {
    Show(String),
    Clear,
}

pub fn view_logs_panel<'a>(logs: &'a [String]) -> Element<'a, Message> {
    let header = container(
        Text::new("System Logs")
            .size(32)
            .style(styles::header_text)
    )
    .padding(20)
    .style(styles::panel_content);

    let logs_list = container(
        column(
            logs.iter().map(|log| {
                container(
                    Text::new(log).size(14)
                )
                .into()
            }).collect::<Vec<Element<'a, Message>>>()
        )
        .spacing(5)
    )
    .padding(10)
    .style(styles::panel_content);

    let clear_button = common::primary_button("Clear", 14)
        .on_press(Message::LogMessage(LogMessage::Clear));

    container(
        column![
            header,
            logs_list,
            clear_button
        ]
        .spacing(20)
    )
    .padding(20)
    .style(styles::panel_content)
    .into()
} 