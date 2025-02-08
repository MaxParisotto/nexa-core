use iced::widget::{button, column, container, row, text_input, Text};
use iced::Element;
use crate::cli::{AgentAction, AgentWorkflow, WorkflowStep, WorkflowStatus};
use super::{common, styles};

#[derive(Debug, Clone)]
pub enum WorkflowMessage {
    UpdateName(String),
    AddStep(WorkflowStep),
    RemoveStep(usize),
    Create(String, Vec<WorkflowStep>),
    Execute(String),
    ViewDetails(String),
    StatusChanged(String, WorkflowStatus),
    List,
    Back,
}

pub fn view_workflow_header<'a>() -> Element<'a, WorkflowMessage> {
    common::header("Workflow Management")
}

pub fn view_new_workflow_form<'a>(
    name: &str,
    steps: &[WorkflowStep]
) -> Element<'a, WorkflowMessage> {
    let name_input = text_input(
        "Workflow Name",
        name
    )
    .on_input(WorkflowMessage::UpdateName)
    .padding(10)
    .size(16);

    let steps_list = if !steps.is_empty() {
        column(
            steps.iter().enumerate().map(|(index, step)| {
                view_workflow_step(index, step)
            }).collect::<Vec<Element<WorkflowMessage>>>()
        )
    } else {
        column![
            Text::new("No steps added yet").size(14)
        ]
    };

    let add_step_button = button(Text::new("Add Step").size(16))
        .on_press(WorkflowMessage::AddStep(WorkflowStep {
            agent_id: String::new(),
            action: AgentAction::ProcessText {
                input: String::new(),
                _max_tokens: 100,
            },
            dependencies: Vec::new(),
            retry_policy: None,
            timeout_seconds: None,
        }))
        .padding(10)
        .style(button::secondary);

    let create_button = button(Text::new("Create Workflow").size(16))
        .on_press(WorkflowMessage::Create(
            name.to_string(),
            steps.to_vec()
        ))
        .padding(10)
        .style(button::primary)
        .width(iced::Length::Fill);

    common::section(
        "Create New Workflow",
        column![
            name_input,
            steps_list,
            add_step_button,
            create_button
        ]
        .spacing(20)
        .into()
    )
}

fn view_workflow_step<'a>(index: usize, step: &WorkflowStep) -> Element<'a, WorkflowMessage> {
    let step_text = match &step.action {
        AgentAction::ProcessText { input, .. } => 
            format!("Process Text: {}", input),
        AgentAction::GenerateCode { prompt, language } => 
            format!("Generate {} Code: {}", language, prompt),
        AgentAction::AnalyzeCode { code, aspects } => 
            format!("Analyze Code: {} ({})", code, aspects.join(", ")),
        AgentAction::CustomTask { task_type, parameters } => 
            format!("Custom Task: {} - {:?}", task_type, parameters),
    };

    container(
        row![
            Text::new(format!("Step {}", index + 1)).size(16),
            Text::new(format!("Agent: {}", step.agent_id)).size(16),
            Text::new(step_text).size(14),
            button(Text::new("Remove").size(14))
                .on_press(WorkflowMessage::RemoveStep(index))
                .style(button::danger)
        ]
        .spacing(10)
    )
    .padding(10)
    .style(styles::panel_content)
    .into()
}

pub fn view_workflows_list<'a>(workflows: &'a [AgentWorkflow]) -> Element<'a, WorkflowMessage> {
    let workflows_list = column(
        workflows.iter().map(|workflow| {
            view_workflow_item(workflow)
        }).collect::<Vec<Element<'a, WorkflowMessage>>>()
    )
    .spacing(10);

    common::section("Workflows", workflows_list.into())
}

fn view_workflow_item<'a>(workflow: &'a AgentWorkflow) -> Element<'a, WorkflowMessage> {
    container(
        column![
            row![
                Text::new(&workflow.name)
                    .size(20)
                    .style(styles::header_text),
                Text::new(format!("Steps: {}", workflow.steps.len()))
                    .size(14)
            ]
            .spacing(10),
            view_workflow_steps(&workflow.steps)
        ]
        .spacing(10)
    )
    .padding(10)
    .style(styles::panel_content)
    .into()
}

fn view_workflow_steps<'a>(steps: &'a [WorkflowStep]) -> Element<'a, WorkflowMessage> {
    column(
        steps.iter().enumerate().map(|(index, step)| {
            row![
                Text::new(format!("{}. Agent: {}", index + 1, step.agent_id))
                    .size(14)
            ]
            .spacing(10)
            .into()
        }).collect::<Vec<Element<'a, WorkflowMessage>>>()
    )
    .spacing(5)
    .into()
}

pub fn view_workflow_details<'a>(workflow: &'a AgentWorkflow) -> Element<'a, WorkflowMessage> {
    let header = container(
        Text::new(format!("Workflow Details: {}", workflow.name))
            .size(32)
            .style(styles::header_text)
    )
    .padding(20)
    .style(styles::panel_content);

    let details = container(
        column![
            row![
                Text::new("Status: ").size(16),
                Text::new(format!("{:?}", workflow.status)).size(16)
            ],
            row![
                Text::new("Steps: ").size(16),
                Text::new(format!("{}", workflow.steps.len())).size(16)
            ],
            if let Some(last_run) = workflow.last_run {
                row![
                    Text::new("Last Run: ").size(16),
                    Text::new(last_run.to_string()).size(16)
                ]
            } else {
                row![
                    Text::new("Last Run: ").size(16),
                    Text::new("Never").size(16)
                ]
            }
        ]
        .spacing(10)
    )
    .padding(20)
    .style(styles::panel_content);

    column![
        header,
        details,
        view_workflow_steps(&workflow.steps)
    ]
    .spacing(20)
    .into()
} 