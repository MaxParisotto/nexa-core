use iced::{Border, Color, Shadow, Theme};
use iced::widget::{container, text};
use crate::models::agent::AgentStatus;

// Modern color palette with semantic naming
pub struct ThemeColors {
    pub background: Color,
    pub surface: Color,
    pub surface_dark: Color,
    pub border: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub accent: Color,
}

impl ThemeColors {
    pub fn dark() -> Self {
        Self {
            background: Color::from_rgb(0.12, 0.14, 0.18), // Dark blue-gray
            surface: Color::from_rgb(0.16, 0.18, 0.24),    // Slightly lighter blue-gray
            surface_dark: Color::from_rgb(0.10, 0.12, 0.16), // Darker blue-gray
            border: Color::from_rgb(0.25, 0.28, 0.32),     // Medium gray
            text: Color::from_rgb(0.90, 0.92, 0.95),       // Light gray
            text_secondary: Color::from_rgb(0.70, 0.72, 0.75), // Medium light gray
            accent: Color::from_rgb(0.20, 0.50, 0.95),     // Bright blue
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
    let colors = ThemeColors::dark();
    
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
    let colors = ThemeColors::dark();
    
    container::Style {
        background: Some(colors.surface_dark.into()),
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
    let colors = ThemeColors::dark();
    
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
    let colors = ThemeColors::dark();
    
    text::Style {
        color: Some(colors.text),
        ..Default::default()
    }
}

pub fn panel_content(_theme: &Theme) -> container::Style {
    let colors = ThemeColors::dark();

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
    let colors = ThemeColors::dark();

    container::Style {
        background: Some(colors.surface_dark.into()),
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
            AgentStatus::Active => Color::from_rgb(0.2, 0.8, 0.4),  // Bright green
            AgentStatus::Idle => Color::from_rgb(0.9, 0.7, 0.2),    // Bright yellow
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