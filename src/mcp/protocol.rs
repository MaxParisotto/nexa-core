use chrono::{DateTime, Utc};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::types::{Agent, Task, AgentStatus, TaskStatus};
use crate::error::NexaError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageType {
    Registration,
    TaskAssignment,
    TaskUpdate,
    StatusUpdate,
    AgentQuery,
    AgentResponse,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub message_type: MessageType,
    pub timestamp: DateTime<Utc>,
    pub sender_id: String,
    #[serde(flatten)]
    pub payload: MessagePayload,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MessagePayload {
    Registration(RegistrationPayload),
    TaskAssignment(TaskAssignmentPayload),
    TaskUpdate(TaskUpdatePayload),
    StatusUpdate(StatusUpdatePayload),
    AgentQuery(AgentQueryPayload),
    AgentResponse(AgentResponsePayload),
    Error(ErrorPayload),
    CodeGeneration(CodeGenerationTask),
    CodeResult(CodeGenerationResult),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrationPayload {
    pub agent: Agent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskAssignmentPayload {
    pub task: Task,
    pub agent_id: String,
    pub deadline: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskUpdatePayload {
    pub task_id: String,
    pub status: TaskStatus,
    pub progress: Option<f32>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusUpdatePayload {
    pub agent_id: String,
    pub status: AgentStatus,
    pub metrics: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentQueryPayload {
    pub capability_type: String,
    pub requirements: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResponsePayload {
    pub agents: Vec<Agent>,
    pub query_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: u32,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodeGenerationTask {
    pub prompt: String,
    pub language: String,
    pub requirements: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodeGenerationResult {
    pub code: String,
    pub language: String,
    pub metrics: Option<HashMap<String, String>>,
}

impl Message {
    pub fn new(message_type: MessageType, sender_id: String, payload: MessagePayload) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            message_type,
            timestamp: Utc::now(),
            sender_id,
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), NexaError> {
        match &self.payload {
            MessagePayload::Registration(p) => Self::validate_registration(p),
            MessagePayload::TaskAssignment(p) => Self::validate_task_assignment(p),
            MessagePayload::TaskUpdate(p) => Self::validate_task_update(p),
            _ => Ok(()),
        }
    }

    fn validate_registration(payload: &RegistrationPayload) -> Result<(), NexaError> {
        if payload.agent.id.is_empty() {
            return Err(NexaError::Protocol("Agent ID cannot be empty".to_string()));
        }
        Ok(())
    }

    fn validate_task_assignment(payload: &TaskAssignmentPayload) -> Result<(), NexaError> {
        if payload.agent_id.is_empty() {
            return Err(NexaError::Protocol("Agent ID cannot be empty".to_string()));
        }
        Ok(())
    }

    fn validate_task_update(payload: &TaskUpdatePayload) -> Result<(), NexaError> {
        if payload.task_id.is_empty() {
            return Err(NexaError::Protocol("Task ID cannot be empty".to_string()));
        }
        if let Some(progress) = payload.progress {
            if progress < 0.0 || progress > 100.0 {
                return Err(NexaError::Protocol("Progress must be between 0 and 100".to_string()));
            }
        }
        Ok(())
    }
}

/// Handles MCP protocol operations and message processing
#[derive(Debug, Clone)]
pub struct ProtocolHandler {
    // Protocol state
    active: bool,
}

impl ProtocolHandler {
    pub fn new() -> Self {
        Self {
            active: false,
        }
    }

    pub fn handle_message(&self, _message: super::MCPMessage) -> Result<(), NexaError> {
        // Protocol message handling implementation
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_message_serialization() {
        let payload = RegistrationPayload {
            agent: Agent {
                id: "agent-1".to_string(),
                name: "Test Agent".to_string(),
                capabilities: vec![],
                status: AgentStatus::Idle,
                current_task: None,
                last_heartbeat: Utc::now(),
            },
        };

        let message = Message::new(
            MessageType::Registration,
            "sender-1".to_string(),
            MessagePayload::Registration(payload),
        );

        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: Message = serde_json::from_str(&serialized).unwrap();

        assert_eq!(message.sender_id, deserialized.sender_id);
    }

    #[test]
    fn test_message_validation() {
        let payload = TaskUpdatePayload {
            task_id: "".to_string(),
            status: TaskStatus::InProgress,
            progress: Some(150.0),
            message: None,
        };

        let message = Message::new(
            MessageType::TaskUpdate,
            "sender-1".to_string(),
            MessagePayload::TaskUpdate(payload),
        );

        assert!(message.validate().is_err());
    }
}
