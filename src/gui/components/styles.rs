use iced::{Border, Color, Shadow, Theme};
use iced::widget::{container, text};
use crate::models::agent::AgentStatus;

// Modern color palette with semantic naming
pub struct ThemeColors {
    pub background: Color,
    pub surface: Color,
    pub border: Color,
    pub text: Color,
}

impl ThemeColors {
    pub fn light() -> Self {
        Self {
            background: Color::from_rgb(0.98, 0.99, 1.00),
            surface: Color::from_rgb(1.0, 1.0, 1.0),
            border: Color::from_rgb(0.90, 0.92, 0.95),
            text: Color::from_rgb(0.15, 0.18, 0.20),
        }
    }
}

// Reusable shadow definitions
pub struct Shadows {
    pub small: Shadow,
    pub medium: Shadow,
    pub large: Shadow,
}

impl Shadows {
    pub fn new() -> Self {
        Self {
            small: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.08),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            medium: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.12),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 16.0,
            },
            large: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.16),
                offset: iced::Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
        }
    }
}

pub fn dock_item(_theme: &Theme) -> container::Style {
    let colors = ThemeColors::light();
    
    container::Style {
        background: Some(colors.surface.into()),
        border: Border {
            width: 1.0,
            color: colors.border,
            radius: (12.0).into(),
        },
        shadow: Shadows::new().medium,
        text_color: Some(colors.text),
        ..Default::default()
    }
}

pub fn dock(_theme: &Theme) -> container::Style {
    let colors = ThemeColors::light();
    
    container::Style {
        background: Some(colors.surface.into()),
        border: Border {
            width: 1.0,
            color: colors.border,
            radius: (20.0).into(),
        },
        shadow: Shadows::new().large,
        ..Default::default()
    }
}

pub fn main_container(_theme: &Theme) -> container::Style {
    let colors = ThemeColors::light();
    
    container::Style {
        background: Some(colors.background.into()),
        border: Border {
            width: 1.0,
            color: colors.border,
            radius: (16.0).into(),
        },
        shadow: Shadows::new().medium,
        ..Default::default()
    }
}

pub fn header_text(_theme: &Theme) -> text::Style {
    let colors = ThemeColors::light();
    
    text::Style {
        color: Some(colors.text),
        ..Default::default()
    }
}

pub fn panel_content(_theme: &Theme) -> container::Style {
    let colors = ThemeColors::light();

    container::Style {
        background: Some(colors.surface.into()),
        border: Border {
            width: 1.0,
            color: colors.border,
            radius: (12.0).into(),
        },
        shadow: Shadows::new().small,
        ..Default::default()
    }
}

pub fn search_bar(_theme: &Theme) -> container::Style {
    let colors = ThemeColors::light();

    container::Style {
        background: Some(colors.surface.into()),
        border: Border {
            width: 1.0,
            color: colors.border,
            radius: (10.0).into(),
        },
        shadow: Shadows::new().small,
        ..Default::default()
    }
}

pub fn status_badge_style(status: AgentStatus) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| {
        let color = match status {
            AgentStatus::Active => Color::from_rgb(0.2, 0.8, 0.4),  // Green
            AgentStatus::Idle => Color::from_rgb(0.9, 0.7, 0.2),    // Yellow
        };

        container::Style {
            background: Some(color.into()),
            border: Border {
                width: 0.0,
                color: Color::TRANSPARENT,
                radius: (4.0).into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(color.r, color.g, color.b, 0.3),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 4.0,
            },
            ..Default::default()
        }
    }
} 