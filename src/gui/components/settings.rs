use iced::widget::{button, column, container, row, text_input, Text};
use iced::Element;
use crate::cli::LLMModel;
use crate::settings::LLMServerConfig;
use crate::gui::components::{common, styles};

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    AddServer(String, String),
    RemoveServer(String),
    Connect(String),
    Disconnect(String),
    UpdateNewServerUrl(String),
    UpdateNewServerProvider(String),
    SelectModel(String, String),
    ModelsLoaded(String, Vec<LLMModel>),
}

pub fn view_settings_header<'a>() -> Element<'a, SettingsMessage> {
    common::header("LLM Settings")
}

pub fn view_add_server_form<'a>(
    new_server_url: &'a str,
    new_server_provider: &'a str,
) -> Element<'a, SettingsMessage> {
    let provider_input = text_input(
        "Provider (e.g. LMStudio, Ollama)",
        new_server_provider
    )
    .on_input(SettingsMessage::UpdateNewServerProvider)
    .padding(10)
    .size(16);

    let url_input = text_input(
        "Server URL",
        new_server_url
    )
    .on_input(SettingsMessage::UpdateNewServerUrl)
    .padding(10)
    .size(16);

    let add_button = button(Text::new("Add Server").size(16))
        .on_press(SettingsMessage::AddServer(
            new_server_url.to_string(),
            new_server_provider.to_string()
        ))
        .padding(10)
        .style(button::primary);

    common::section(
        "Add New LLM Server",
        row![
            provider_input,
            url_input,
            add_button
        ]
        .spacing(15)
        .into()
    )
}

pub fn view_servers_list<'a>(
    servers: &'a [LLMServerConfig],
    available_models: &'a [LLMModel]
) -> Element<'a, SettingsMessage> {
    let servers_list = column(
        servers.iter().map(|server| {
            view_server_item(server)
        }).collect::<Vec<_>>()
    )
    .spacing(10);

    let models_list = column(
        available_models.iter().map(|model| {
            view_model_item(model)
        }).collect::<Vec<_>>()
    )
    .spacing(10);

    column![
        common::section("LLM Servers", servers_list.into()),
        common::section("Available Models", models_list.into())
    ]
    .spacing(20)
    .into()
}

fn view_server_item<'a>(server: &'a LLMServerConfig) -> Element<'a, SettingsMessage> {
    let connect_button = button(
        Text::new(if server.url.is_empty() { "Connect" } else { "Disconnect" }).size(14)
    )
    .on_press(SettingsMessage::Connect(server.provider.clone()))
    .padding(10)
    .style(if server.url.is_empty() { button::primary } else { button::danger });

    container(
        row![
            Text::new(&server.provider)
                .size(16),
            Text::new(&server.url)
                .size(14),
            connect_button
        ]
        .spacing(10)
    )
    .padding(10)
    .style(styles::panel_content)
    .into()
}

fn view_model_item<'a>(model: &'a LLMModel) -> Element<'a, SettingsMessage> {
    container(
        row![
            Text::new(&model.name)
                .size(16),
            Text::new(format!("Context: {} tokens", model.context_length))
                .size(14)
        ]
        .spacing(10)
    )
    .padding(10)
    .style(styles::panel_content)
    .into()
} 