use iced::{
    widget::{self, container, button},
    Color, Theme, Vector, Background,
    theme::{self, Container as ThemeContainer, Button as ThemeButton},
};
use iced::widget::container::{StyleSheet as ContainerStyleSheet, Appearance as ContainerAppearance};
use iced::widget::button::{StyleSheet as ButtonStyleSheet, Appearance as ButtonAppearance};
use iced::widget::Container;
use iced::widget::container::{Appearance, StyleSheet};
use iced::application::StyleSheet as ApplicationStyleSheet;
use iced::widget::button::StyleSheet as ButtonStyleSheetTrait;
use iced::widget::container::StyleSheet as ContainerStyleSheetTrait;
use crate::gui::types::Message;

// Custom theme colors
const BACKGROUND: Color = Color::from_rgb(
    0x1E as f32 / 255.0,
    0x1E as f32 / 255.0,
    0x1E as f32 / 255.0,
);

const SURFACE: Color = Color::from_rgb(
    0x2D as f32 / 255.0,
    0x2D as f32 / 255.0,
    0x2D as f32 / 255.0,
);

const ACCENT: Color = Color::from_rgb(
    0x6F as f32 / 255.0,
    0x1D as f32 / 255.0,
    0xF7 as f32 / 255.0,
);

const ACTIVE: Color = Color::from_rgb(
    0x72 as f32 / 255.0,
    0x89 as f32 / 255.0,
    0xDA as f32 / 255.0,
);

const TEXT: Color = Color::from_rgb(0.9, 0.9, 0.9);

pub struct MainContainer;
pub struct ContentContainer;
pub struct SidebarContainer;
pub struct CardContainer;

impl container::StyleSheet for MainContainer {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: None,
            text_color: None,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Color::BLACK,
        }
    }
}

impl container::StyleSheet for ContentContainer {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> container::Appearance {
        container::Appearance {
            text_color: Some(TEXT),
            background: Some(SURFACE.into()),
            border_radius: 8.0,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

impl container::StyleSheet for SidebarContainer {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> container::Appearance {
        container::Appearance {
            text_color: Some(TEXT),
            background: Some(SURFACE.into()),
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }
}

impl container::StyleSheet for CardContainer {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> container::Appearance {
        container::Appearance {
            text_color: Some(TEXT),
            background: Some(BACKGROUND.into()),
            border_radius: 8.0,
            border_width: 1.0,
            border_color: Color::from_rgb(0.3, 0.3, 0.3),
        }
    }
}

pub fn custom_theme() -> Theme {
    Theme::custom(theme::Palette {
        background: BACKGROUND,
        text: TEXT,
        primary: ACCENT,
        success: Color::from_rgb(0.0, 0.8, 0.4),
        danger: Color::from_rgb(0.8, 0.2, 0.2),
    })
}

// Container styles
pub fn container_style(appearance: theme::Container) -> Container {
    Container::Custom(Box::new(appearance))
}

pub fn main_container() -> ThemeContainer {
    ThemeContainer::Custom(Box::new(|_theme: &Theme| theme::Container {
        background: None,
        text_color: None,
        border_radius: 0.0,
        border_width: 0.0,
        border_color: Color::BLACK,
    }))
}

pub fn content_container() -> ThemeContainer {
    ThemeContainer::Custom(Box::new(|_theme: &Theme| theme::Container {
        text_color: Some(TEXT),
        background: Some(SURFACE.into()),
        border_radius: 8.0,
        border_width: 0.0,
        border_color: Color::TRANSPARENT,
    }))
}

pub fn sidebar_container() -> ThemeContainer {
    ThemeContainer::Custom(Box::new(|_theme: &Theme| theme::Container {
        text_color: Some(TEXT),
        background: Some(SURFACE.into()),
        border_radius: 0.0,
        border_width: 0.0,
        border_color: Color::TRANSPARENT,
    }))
}

pub fn card_container() -> ThemeContainer {
    ThemeContainer::Custom(Box::new(|_theme: &Theme| theme::Container {
        text_color: Some(TEXT),
        background: Some(BACKGROUND.into()),
        border_radius: 8.0,
        border_width: 1.0,
        border_color: Color::from_rgb(0.3, 0.3, 0.3),
    }))
}

// Button styles
pub fn primary_button() -> ThemeButton {
    ThemeButton::Custom(Box::new(|_theme: &Theme| theme::Button {
        text_color: Color::WHITE,
        background: Some(Color::from_rgb(0.0, 0.5, 1.0).into()),
        border_radius: 5.0,
        shadow_offset: Vector::new(1.0, 1.0),
        border_width: 1.0,
        border_color: Color::TRANSPARENT,
    }))
}

pub fn secondary_button() -> ThemeButton {
    ThemeButton::Custom(Box::new(|_theme: &Theme| theme::Button {
        text_color: Color::BLACK,
        background: Some(Color::from_rgb(0.9, 0.9, 0.9).into()),
        border_radius: 5.0,
        shadow_offset: Vector::new(1.0, 1.0),
        border_width: 1.0,
        border_color: Color::TRANSPARENT,
    }))
}

pub struct ModernCard;
pub struct SidebarStyle;
pub struct ErrorLogStyle;

impl StyleSheet for ModernCard {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> Appearance {
        Appearance {
            background: Some(Background::Color(Color::WHITE)),
            border_radius: 8.0,
            border_width: 1.0,
            border_color: Color::BLACK,
            text_color: None,
        }
    }
}

impl StyleSheet for SidebarStyle {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> Appearance {
        Appearance {
            background: Some(Background::Color(Color::from_rgb(0.15, 0.15, 0.15))),
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            text_color: None,
        }
    }
}

impl StyleSheet for ErrorLogStyle {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> Appearance {
        Appearance {
            background: Some(Background::Color(Color::from_rgb(1.0, 0.8, 0.8))),
            border_radius: 4.0,
            border_width: 1.0,
            border_color: Color::from_rgb(0.8, 0.0, 0.0),
            text_color: None,
        }
    }
}

impl<'a> From<ModernCard> for Container<'a, Message> {
    fn from(style: ModernCard) -> Self {
        Container::new(iced::widget::Text::new("")).style(style)
    }
}

impl<'a> From<SidebarStyle> for Container<'a, Message> {
    fn from(style: SidebarStyle) -> Self {
        Container::new(iced::widget::Text::new("")).style(style)
    }
}

impl<'a> From<ErrorLogStyle> for Container<'a, Message> {
    fn from(style: ErrorLogStyle) -> Self {
        Container::new(iced::widget::Text::new("")).style(style)
    }
}

impl<'a> From<MainContainer> for Container<'a, Message> {
    fn from(style: MainContainer) -> Self {
        Container::new(iced::widget::Text::new("")).style(style)
    }
}

impl<'a> From<ContentContainer> for Container<'a, Message> {
    fn from(style: ContentContainer) -> Self {
        Container::new(iced::widget::Text::new("")).style(style)
    }
}

impl<'a> From<SidebarContainer> for Container<'a, Message> {
    fn from(style: SidebarContainer) -> Self {
        Container::new(iced::widget::Text::new("")).style(style)
    }
}

impl<'a> From<CardContainer> for Container<'a, Message> {
    fn from(style: CardContainer) -> Self {
        Container::new(iced::widget::Text::new("")).style(style)
    }
} 