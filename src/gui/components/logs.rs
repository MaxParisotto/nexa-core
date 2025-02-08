use iced::widget::{column, container, scrollable, Text};
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
        scrollable(
            column(
                logs.iter().map(|log| {
                    container(
                        Text::new(log)
                            .size(14)
                            .style(styles::header_text)
                    )
                    .padding(5)
                    .width(iced::Length::Fill)
                    .style(styles::panel_content)
                    .into()
                }).collect::<Vec<Element<'a, Message>>>()
            )
            .spacing(5)
            .width(iced::Length::Fill)
        )
        .height(iced::Length::Fill)
    )
    .height(iced::Length::Fill)
    .padding(10)
    .style(styles::panel_content);

    let clear_button = common::primary_button("Clear Logs", 14)
        .on_press(Message::LogMessage(LogMessage::Clear))
        .width(iced::Length::Fill);

    container(
        column![
            header,
            logs_list,
            clear_button
        ]
        .spacing(20)
        .height(iced::Length::Fill)
    )
    .padding(20)
    .height(iced::Length::Fill)
    .style(styles::panel_content)
    .into()
} 