use iced::{Border, Color, Shadow, Theme, Vector};
use iced::widget::{container, text};
use crate::models::agent::AgentStatus;
use log::debug;

/// Modern color palette with semantic naming using dynamic, modern aesthetics.
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
    /// Returns a modern dark theme color set.
    pub fn modern_dark() -> Self {
        Self {
            background: Color::from_rgb(0.05, 0.05, 0.07), // Very dark background
            surface: Color::from_rgb(0.12, 0.12, 0.15),    // Dark surface
            surface_dark: Color::from_rgb(0.08, 0.08, 0.10), // Even darker surface
            border: Color::from_rgb(0.30, 0.30, 0.35),       // Subtle border tone
            text: Color::from_rgb(0.95, 0.95, 0.98),         // Crisp light text
            text_secondary: Color::from_rgb(0.70, 0.70, 0.75), // Muted secondary text
            accent: Color::from_rgb(0.40, 0.80, 1.0),         // Vibrant neon accent
        }
    }
}

/// Modern shadow definitions with enhanced blur and offset for a dynamic look.
pub struct Shadows {
    pub small: Shadow,
    pub medium: Shadow,
    pub large: Shadow,
}

impl Shadows {
    /// Create new dynamic shadows.
    pub fn new() -> Self {
        Self {
            small: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
                offset: Vector::new(0.0, 2.0),
                blur_radius: 10.0,
            },
            medium: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.15),
                offset: Vector::new(0.0, 4.0),
                blur_radius: 20.0,
            },
            large: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.20),
                offset: Vector::new(0.0, 6.0),
                blur_radius: 30.0,
            },
        }
    }
}

/// Style for dock items in the UI navigation panel.
pub fn dock_item(_theme: &Theme) -> container::Style {
    debug!("Applying modern dock_item style");
    let colors = ThemeColors::modern_dark();
    container::Style {
        background: Some(colors.surface.into()),
        border: Border {
            width: 2.0,
            color: colors.accent,
            radius: 16.0.into(),
        },
        shadow: Shadows::new().medium,
        text_color: Some(colors.text),
        ..Default::default()
    }
}

/// Style for the dock container.
pub fn dock(_theme: &Theme) -> container::Style {
    debug!("Applying modern dock style");
    let colors = ThemeColors::modern_dark();
    container::Style {
        background: Some(colors.surface_dark.into()),
        border: Border {
            width: 2.0,
            color: colors.accent,
            radius: 24.0.into(),
        },
        shadow: Shadows::new().large,
        text_color: Some(colors.text),
        ..Default::default()
    }
}

/// Style for the main container that holds the primary UI content.
pub fn main_container(_theme: &Theme) -> container::Style {
    debug!("Applying modern main_container style");
    let colors = ThemeColors::modern_dark();
    container::Style {
        background: Some(colors.background.into()),
        border: Border {
            width: 2.0,
            color: colors.border,
            radius: 20.0.into(),
        },
        shadow: Shadows::new().medium,
        ..Default::default()
    }
}

pub fn search_bar(_theme: &Theme) -> container::Style {
    let colors = ThemeColors::modern_dark();

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

/// Returns the header text style for prominent headings.
pub fn header_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(ThemeColors::modern_dark().text),
        ..Default::default()
    }
}

/// Returns the style for panel containers (sections and panels).
pub fn panel_content(_theme: &Theme) -> container::Style {
    let colors = ThemeColors::modern_dark();
    container::Style {
        background: Some(colors.surface.into()),
        border: Border {
            width: 2.0,
            color: colors.border,
            radius: 12.0.into(),
        },
        text_color: Some(colors.text),
        ..Default::default()
    }
} 