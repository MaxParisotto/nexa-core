use iced::widget::{button, column, container, row, text_input, Text};
use iced::Element;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::models::agent::Task;
use crate::models::agent::TaskStatus;
use super::{common, styles};

#[derive(Debug, Clone)]
pub enum TaskMessage {
    UpdateTitle(String),
    UpdateDescription(String),
    UpdatePriority(i32),
    UpdateAgent(Option<String>),
    UpdateDeadline(Option<DateTime<Utc>>),
    UpdateDuration(i64),
    Create(Task),
    ViewDetails(String),
    AssignTask(String, String),
    UpdateStatus(String, TaskStatus),
    Delete(String),
    List,
    Back,
}

pub fn view_task_header<'a>() -> Element<'a, TaskMessage> {
    common::header("Task Management")
}

pub fn view_new_task_form<'a>(
    title: &str,
    description: &str,
    priority: i32,
    duration: i64,
) -> Element<'a, TaskMessage> {
    let title_input = text_input(
        "Task Title",
        title
    )
    .on_input(TaskMessage::UpdateTitle)
    .padding(10)
    .size(16);

    let description_input = text_input(
        "Task Description",
        description
    )
    .on_input(TaskMessage::UpdateDescription)
    .padding(10)
    .size(16);

    let priority_input = text_input(
        "50",
        &priority.to_string()
    )
    .on_input(|s| TaskMessage::UpdatePriority(s.parse().unwrap_or(50)))
    .padding(10)
    .size(16);

    let duration_input = text_input(
        "3600",
        &duration.to_string()
    )
    .on_input(|s| TaskMessage::UpdateDuration(s.parse().unwrap_or(3600)))
    .padding(10)
    .size(16);

    let create_button = button(Text::new("Create Task").size(16))
        .on_press(TaskMessage::Create(Task {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
            steps: Vec::new(),
            requirements: Vec::new(),
            assigned_agent: None,
            created_at: Utc::now(),
            deadline: None,
            priority,
            estimated_duration: duration,
        }))
        .padding(10)
        .style(button::primary)
        .width(iced::Length::Fill);

    common::section(
        "Create New Task",
        column![
            title_input,
            description_input,
            row![
                Text::new("Priority (0-100): ").size(16),
                priority_input
            ],
            row![
                Text::new("Estimated Duration (seconds): ").size(16),
                duration_input
            ],
            create_button
        ]
        .spacing(20)
        .into()
    )
}

pub fn view_tasks_list<'a>(tasks: &'a [Task]) -> Element<'a, TaskMessage> {
    let tasks_list = column(
        tasks.iter().map(|task| {
            view_task_item(task)
        }).collect::<Vec<Element<'a, TaskMessage>>>()
    )
    .spacing(20);

    common::section(
        "Active Tasks",
        tasks_list.into()
    )
}

fn view_task_item<'a>(task: &'a Task) -> Element<'a, TaskMessage> {
    container(
        column![
            row![
                Text::new(&task.title).size(20).style(styles::header_text),
                Text::new(format!("Status: {:?}", task.status)).size(14)
            ]
            .spacing(10),
            Text::new(&task.description).size(14)
        ]
        .spacing(10)
    )
    .padding(10)
    .style(styles::panel_content)
    .into()
}

pub fn view_task_details<'a>(task: &'a Task) -> Element<'a, TaskMessage> {
    let header = container(
        Text::new(format!("Task Details: {}", task.title))
            .size(32)
            .style(styles::header_text)
    )
    .padding(20)
    .style(styles::panel_content);

    let details = container(
        column![
            row![
                Text::new("Status: ").size(16).style(styles::header_text),
                Text::new(format!("{:?}", task.status)).size(16)
            ],
            row![
                Text::new("Description: ").size(16).style(styles::header_text),
                Text::new(&task.description).size(16)
            ],
            row![
                Text::new("Priority: ").size(16).style(styles::header_text),
                Text::new(format!("{}", task.priority)).size(16)
            ],
            row![
                Text::new("Estimated Duration: ").size(16).style(styles::header_text),
                Text::new(format!("{} seconds", task.estimated_duration)).size(16)
            ],
            row![
                Text::new("Created: ").size(16).style(styles::header_text),
                Text::new(task.created_at.format("%Y-%m-%d %H:%M:%S").to_string()).size(16)
            ],
            row![
                Text::new("Deadline: ").size(16).style(styles::header_text),
                if let Some(deadline) = task.deadline {
                    Text::new(deadline.format("%Y-%m-%d %H:%M:%S").to_string()).size(16)
                } else {
                    Text::new("None").size(16)
                }
            ]
        ]
        .spacing(15)
    )
    .padding(20)
    .style(styles::panel_content);

    let action_buttons = container(
        row![
            button(Text::new("Start Task").size(16))
                .on_press(TaskMessage::UpdateStatus(task.id.clone(), TaskStatus::InProgress))
                .padding(10)
                .style(button::primary),
            button(Text::new("Complete Task").size(16))
                .on_press(TaskMessage::UpdateStatus(task.id.clone(), TaskStatus::Completed))
                .padding(10)
                .style(button::primary),
            button(Text::new("Mark Failed").size(16))
                .on_press(TaskMessage::UpdateStatus(task.id.clone(), TaskStatus::Failed))
                .padding(10)
                .style(button::danger),
            button(Text::new("Delete Task").size(16))
                .on_press(TaskMessage::Delete(task.id.clone()))
                .padding(10)
                .style(button::danger),
            button(Text::new("Back to List").size(16))
                .on_press(TaskMessage::Back)
                .padding(10)
                .style(button::secondary)
        ]
        .spacing(15)
    )
    .padding(20)
    .style(styles::panel_content);

    column![
        header,
        details,
        action_buttons
    ]
    .spacing(20)
    .into()
} 