use iced::widget::{button, container, Text};
use iced::Element;
use super::styles;
use crate::gui::app;

pub fn header<'a, Message: 'a>(title: &'a str) -> Element<'a, Message> {
    container(
        Text::new(title)
            .size(32)
            .style(|theme| styles::header_text(theme))
    )
    .padding(20)
    .style(|theme| styles::panel_content(theme))
    .into()
}

pub fn section<'a, Message: 'a>(title: &'a str, content: Element<'a, Message>) -> Element<'a, Message> {
    container(
        iced::widget::column![
            Text::new(title)
                .size(24)
                .style(|theme| styles::header_text(theme)),
            content
        ]
        .spacing(20)
    )
    .padding(20)
    .style(|theme| styles::panel_content(theme))
    .into()
}

pub fn primary_button<'a>(label: &'a str, size: u16) -> button::Button<'a, app::Message> {
    button(
        Text::new(label).size(size)
    )
    .padding(10)
    .style(button::primary)
}

pub fn secondary_button<'a>(label: &'a str, size: u16) -> button::Button<'a, app::Message> {
    button(
        Text::new(label).size(size)
    )
    .padding(10)
    .style(button::secondary)
}

pub fn danger_button<'a>(label: &'a str, size: u16) -> button::Button<'a, app::Message> {
    button(
        Text::new(label).size(size)
    )
    .padding(10)
    .style(button::danger)
} 