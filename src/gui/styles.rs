use iced::{
    widget::container,
    Color, Background, Border, Theme,
};

#[derive(Debug, Clone, Copy)]
pub struct ModernCard;

impl From<ModernCard> for iced::theme::Container {
    fn from(_: ModernCard) -> Self {
        iced::theme::Container::Custom(Box::new(ModernCard))
    }
}

impl container::StyleSheet for ModernCard {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
            text_color: Some(Color::BLACK),
            border: Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.8, 0.8, 0.8),
            },
            shadow: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SidebarStyle;

impl From<SidebarStyle> for iced::theme::Container {
    fn from(_: SidebarStyle) -> Self {
        iced::theme::Container::Custom(Box::new(SidebarStyle))
    }
}

impl container::StyleSheet for SidebarStyle {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(Color::from_rgb(0.0, 0.5, 0.7))),
            text_color: Some(Color::WHITE),
            border: Border {
                radius: 0.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ErrorLogStyle;

impl From<ErrorLogStyle> for iced::theme::Container {
    fn from(_: ErrorLogStyle) -> Self {
        iced::theme::Container::Custom(Box::new(ErrorLogStyle))
    }
}

impl container::StyleSheet for ErrorLogStyle {
    type Style = Theme;

    fn appearance(&self, _theme: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(Color::from_rgb(0.9, 0.8, 0.8))),
            text_color: Some(Color::BLACK),
            border: Border {
                radius: 4.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Default::default(),
        }
    }
} 