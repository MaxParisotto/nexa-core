use eframe::egui;
use crate::server::Server;
use std::sync::Arc;
use tokio::runtime::Runtime;

pub struct NexaApp {
    server_status: String,
    server: Arc<Server>,
    runtime: Runtime,
}

impl NexaApp {
    pub fn new(server: Arc<Server>) -> Self {
        Self {
            server_status: "Connected".to_string(),
            server,
            runtime: Runtime::new().unwrap(),
        }
    }
}

impl eframe::App for NexaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Update server status
            let server_state = self.runtime.block_on(async {
                self.server.get_state().await
            });
            self.server_status = format!("{:?}", server_state);

            // Title bar
            ui.horizontal(|ui| {
                ui.heading("Nexa Core Server");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let status_color = match server_state {
                        crate::server::ServerState::Running => egui::Color32::GREEN,
                        crate::server::ServerState::Stopped => egui::Color32::RED,
                        crate::server::ServerState::Starting => egui::Color32::YELLOW,
                        crate::server::ServerState::Stopping => egui::Color32::YELLOW,
                    };
                    ui.colored_label(status_color, &self.server_status);
                    ui.label("Status: ");
                });
            });
            ui.separator();

            // Metrics section
            ui.heading("Server Metrics");
            
            // Get metrics using the stored runtime
            let metrics = self.runtime.block_on(async {
                self.server.get_metrics().await
            });

            egui::Grid::new("metrics_grid").striped(true).show(ui, |ui| {
                ui.label("Total Connections:");
                ui.label(metrics.total_connections.to_string());
                ui.end_row();

                ui.label("Active Connections:");
                ui.label(metrics.active_connections.to_string());
                ui.end_row();

                ui.label("Failed Connections:");
                ui.label(metrics.failed_connections.to_string());
                ui.end_row();

                if let Some(error) = &metrics.last_error {
                    ui.label("Last Error:");
                    ui.label(error);
                    ui.end_row();
                }

                ui.label("Uptime:");
                ui.label(format!("{:?}", metrics.uptime));
                ui.end_row();
            });

            // Request continuous updates
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        });
    }
} 