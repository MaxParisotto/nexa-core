use std::time::Duration;
use iced::widget::{container, Column, Row, Text};
use iced::{Element, Length};
use crate::models::agent::{Agent, TaskStatus};
use crate::cli::LLMModel;
use crate::server::ServerMetrics;
use crate::settings::LLMServerConfig;
use crate::gui::components::{common, styles};
use crate::gui::app::Message;
use crate::Task;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct DashboardMetrics {
    // Server metrics
    pub total_connections: u64,
    pub active_connections: u64,
    pub failed_connections: u64,
    pub server_uptime: Duration,
    pub server_last_error: Option<String>,
    
    // Agent metrics
    pub total_agents: usize,
    pub active_agents: usize,
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub average_response_time: f64,
    
    // Task metrics
    pub tasks_pending: usize,
    pub tasks_in_progress: usize,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    
    // LLM metrics
    pub connected_llm_servers: usize,
    pub available_models: usize,
    
    // System metrics
    pub last_update: DateTime<Utc>,
}

impl DashboardMetrics {
    pub fn from_state(
        agents: &[Agent],
        server_metrics: &ServerMetrics,
        llm_servers: &[LLMServerConfig],
        available_models: &[LLMModel],
        tasks: &[Task],
    ) -> Self {
        let active_agents = agents.iter()
            .filter(|a| a.status == crate::models::agent::AgentStatus::Active)
            .count();
            
        // Calculate task metrics from actual tasks
        let tasks_completed = tasks.iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count() as u64;
            
        let tasks_failed = tasks.iter()
            .filter(|t| t.status == TaskStatus::Failed)
            .count() as u64;
            
        // Calculate average response time (if task has a completion time)
        let average_response_time = tasks.iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .filter_map(|t| t.deadline.map(|d| d - t.created_at))
            .map(|d| d.num_milliseconds() as f64)
            .fold((0.0, 0), |(sum, count), time| (sum + time, count + 1));
            
        let average_response_time = if average_response_time.1 > 0 {
            average_response_time.0 / average_response_time.1 as f64
        } else {
            0.0
        };
        
        let tasks_by_status = tasks.iter()
            .fold((0, 0, 0, 0), |acc, task| {
                match task.status {
                    TaskStatus::Pending => (acc.0 + 1, acc.1, acc.2, acc.3),
                    TaskStatus::InProgress => (acc.0, acc.1 + 1, acc.2, acc.3),
                    TaskStatus::Completed => (acc.0, acc.1, acc.2 + 1, acc.3),
                    TaskStatus::Failed => (acc.0, acc.1, acc.2, acc.3 + 1),
                }
            });

        Self {
            total_connections: server_metrics.total_connections,
            active_connections: server_metrics.active_connections as u64,
            failed_connections: server_metrics.failed_connections,
            server_uptime: server_metrics.uptime,
            server_last_error: server_metrics.last_error.clone(),
            
            total_agents: agents.len(),
            active_agents,
            total_tasks_completed: tasks_completed,
            total_tasks_failed: tasks_failed,
            average_response_time,
            
            tasks_pending: tasks_by_status.0,
            tasks_in_progress: tasks_by_status.1,
            tasks_completed: tasks_by_status.2,
            tasks_failed: tasks_by_status.3,
            
            connected_llm_servers: llm_servers.len(),
            available_models: available_models.len(),
            
            last_update: Utc::now(),
        }
    }
}

pub fn view_dashboard<'a>(metrics: DashboardMetrics) -> Element<'a, Message> {
    let server_control = container(
        Column::new()
            .push(Text::new("Server Control").size(24))
            .push(
                Row::new()
                    .push(
                        common::primary_button("Start Server", 16)
                            .on_press(Message::StartServer)
                            .width(Length::Fixed(150.0))
                    )
                    .push(
                        common::danger_button("Stop Server", 16)
                            .on_press(Message::StopServer)
                            .width(Length::Fixed(150.0))
                    )
                    .spacing(20)
                    .padding(10)
            )
            .spacing(10)
    )
    .style(styles::panel_content);

    let server_metrics = container(
        Column::new()
            .push(Text::new("Server Metrics").size(24))
            .push(
                Row::new()
                    .push(metric_card("Total Connections", metrics.total_connections))
                    .push(metric_card("Active Connections", metrics.active_connections))
                    .push(metric_card("Failed Connections", metrics.failed_connections))
                    .spacing(20)
            )
            .push(Text::new(format!("Uptime: {:.2} hours", 
                metrics.server_uptime.as_secs_f64() / 3600.0)))
            .spacing(10)
    )
    .style(styles::panel_content);

    let agent_metrics = container(
        Column::new()
            .push(Text::new("Agent Metrics").size(24))
            .push(
                Row::new()
                    .push(metric_card("Total Agents", metrics.total_agents))
                    .push(metric_card("Active Agents", metrics.active_agents))
                    .spacing(20)
            )
            .push(
                Row::new()
                    .push(metric_card("Tasks Completed", metrics.total_tasks_completed))
                    .push(metric_card("Tasks Failed", metrics.total_tasks_failed))
                    .spacing(20)
            )
            .push(Text::new(format!("Avg Response Time: {:.2}ms", 
                metrics.average_response_time)))
            .spacing(10)
    )
    .style(styles::panel_content);

    let task_metrics = container(
        Column::new()
            .push(Text::new("Task Metrics").size(24))
            .push(
                Row::new()
                    .push(metric_card("Pending", metrics.tasks_pending))
                    .push(metric_card("In Progress", metrics.tasks_in_progress))
                    .push(metric_card("Completed", metrics.tasks_completed))
                    .push(metric_card("Failed", metrics.tasks_failed))
                    .spacing(20)
            )
            .spacing(10)
    )
    .style(styles::panel_content);

    let llm_metrics = container(
        Column::new()
            .push(Text::new("LLM Metrics").size(24))
            .push(
                Row::new()
                    .push(metric_card("Connected Servers", metrics.connected_llm_servers))
                    .push(metric_card("Available Models", metrics.available_models))
                    .spacing(20)
            )
            .spacing(10)
    )
    .style(styles::panel_content);

    let last_update = container(
        Text::new(format!("Last Updated: {}", 
            metrics.last_update.format("%Y-%m-%d %H:%M:%S")))
            .size(12)
    )
    .style(styles::panel_content);

    container(
        Column::new()
            .push(common::header("System Dashboard"))
            .push(server_control)
            .push(server_metrics)
            .push(agent_metrics)
            .push(task_metrics)
            .push(llm_metrics)
            .push(last_update)
            .spacing(20)
    )
    .padding(20)
    .into()
}

fn metric_card<T: std::fmt::Display>(label: &str, value: T) -> Element<Message> {
    container(
        Column::new()
            .push(Text::new(label).size(16))
            .push(Text::new(value.to_string()).size(24))
            .spacing(5)
    )
    .padding(10)
    .style(styles::panel_content)
    .into()
} 