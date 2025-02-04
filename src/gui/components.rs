use iced::{
    widget::{container, text},
    Element, Length,
};

use super::styles::{ModernCard, SidebarStyle, ErrorLogStyle};
use super::types::Message;

pub fn header<'a>(title: &str) -> Element<'a, Message> {
    container(
        text(title)
            .size(24)
    )
    .width(Length::Fill)
    .padding(20)
    .style(ModernCard)
    .into()
}

pub fn sidebar_container<'a, T>(content: T) -> Element<'a, Message> 
where
    T: Into<Element<'a, Message>>,
{
    container(content)
        .width(Length::Fixed(200.0))
        .height(Length::Fill)
        .style(SidebarStyle)
        .into()
}

pub fn error_container<'a, T>(content: T) -> Element<'a, Message>
where
    T: Into<Element<'a, Message>>,
{
    container(content)
        .width(Length::Fill)
        .padding(10)
        .style(ErrorLogStyle)
        .into()
}

pub fn section_container<'a, T>(content: T) -> Element<'a, Message>
where
    T: Into<Element<'a, Message>>,
{
    container(content)
        .width(Length::Fill)
        .padding(20)
        .style(ModernCard)
        .into()
} 