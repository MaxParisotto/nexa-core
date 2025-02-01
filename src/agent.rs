//! Agent Core Types and Traits
//!
//! This module defines the fundamental types and traits for agents:
//! - Agent struct: Core agent representation
//! - AgentCapability: Capability declaration and validation
//! - Task: Task definition and management
//! - Status tracking and updates
//!
//! # Example
//! ```rust
//! use nexa_utils::agent::{Agent, AgentCapability, AgentStatus};
//!
//! let agent = Agent {
//!     id: "agent-1".to_string(),
//!     name: "Example Agent".to_string(),
//!     capabilities: vec![],
//!     status: AgentStatus::Idle,
//!     current_tasks: vec![],
//! };
//! ```

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use utoipa;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub enum AgentStatus {
    Idle,
    Busy,
    Running,
    Error,
    Offline,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Task {
    pub id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub data: serde_json::Value,
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentCapability {
    pub capability_type: String,
    pub parameters: HashMap<String, String>,
    pub required_resources: Vec<String>
}

impl AgentCapability {
    pub fn matches_type(&self, capability_type: &str) -> bool {
        self.capability_type == capability_type
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub capabilities: Vec<AgentCapability>,
    pub status: AgentStatus,
    pub current_tasks: Vec<Task>
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        let agent = Agent {
            id: "test-1".to_string(),
            name: "Test Agent".to_string(),
            capabilities: vec![],
            status: AgentStatus::Idle,
            current_tasks: vec![],
        };
        assert_eq!(agent.id, "test-1");
    }
}
