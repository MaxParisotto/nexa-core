use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use std::time::SystemTime;
use tracing::{error, info, warn};
use serde::{Deserialize, Serialize};
use crate::error::NexaError;
use uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeRole {
    Leader,
    Follower,
    Candidate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeHealth {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug)]
pub struct NodeState {
    pub id: String,
    pub last_heartbeat: SystemTime,
    pub role: NodeRole,
    pub term: u64,
    pub voted_for: Option<String>,
    pub leader_id: Option<String>,
    pub health: NodeHealth,
    pub health_score: f64,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            last_heartbeat: SystemTime::now(),
            role: NodeRole::Follower,
            term: 0,
            voted_for: None,
            leader_id: None,
            health: NodeHealth::Healthy,
            health_score: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterEvent {
    pub event_type: String,
    pub node_id: String,
    pub timestamp: std::time::SystemTime,
    pub data: serde_json::Value,
}

#[derive(Debug)]
pub struct ClusterCoordinator {
    node_id: String,
    peers: Vec<String>,
    state: Arc<RwLock<NodeState>>,
    election_timeout: Duration,
    event_tx: tokio::sync::broadcast::Sender<ClusterEvent>,
    event_rx: tokio::sync::broadcast::Receiver<ClusterEvent>,
}

impl ClusterCoordinator {
    pub fn new(node_id: String, peers: Vec<String>) -> Self {
        let (tx, rx) = tokio::sync::broadcast::channel(100);
        Self {
            node_id,
            peers,
            state: Arc::new(RwLock::new(NodeState::default())),
            election_timeout: Duration::from_millis(5000),
            event_tx: tx,
            event_rx: rx,
        }
    }

    pub async fn start(&self) -> Result<(), NexaError> {
        let node_id = self.node_id.clone();
        let peers = self.peers.clone();
        let state = self.state.clone();
        let election_timeout = self.election_timeout;
        let mut rx = self.event_rx.resubscribe();
        let event_tx = self.event_tx.clone();

        let coordinator = Self {
            node_id,
            peers,
            state,
            election_timeout,
            event_tx,
            event_rx: rx.resubscribe(),
        };

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Ok(event) = rx.recv() => {
                        if let Err(e) = coordinator.handle_event(event).await {
                            error!("Error handling event: {}", e);
                        }
                    }
                    _ = tokio::time::sleep(coordinator.election_timeout) => {
                        if let Err(e) = coordinator.check_election_timeout().await {
                            error!("Error checking election timeout: {}", e);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn check_election_timeout(&self) -> Result<(), NexaError> {
        let state = self.state.read().await;
        let elapsed = SystemTime::now()
            .duration_since(state.last_heartbeat)
            .unwrap_or_default();
        let election_timeout = Duration::from_millis(5000);
        
        if elapsed > election_timeout {
            drop(state);
            let mut state = self.state.write().await;
            state.role = NodeRole::Candidate;
            state.term += 1;
        }
        Ok(())
    }

    pub async fn check_node_health(&self) -> Result<(), NexaError> {
        let state = self.state.read().await;
        let elapsed = SystemTime::now()
            .duration_since(state.last_heartbeat)
            .unwrap_or_default();
        let stale_timeout = Duration::from_millis(15000);
        
        if elapsed > stale_timeout {
            drop(state);
            let mut state = self.state.write().await;
            state.health_score = 0.0;
        }
        Ok(())
    }

    pub async fn send_heartbeat(&self) -> Result<(), NexaError> {
        let state = self.state.read().await;
        if state.role != NodeRole::Leader {
            return Ok(());
        }
        
        let _event = ClusterEvent {
            event_type: "heartbeat".to_string(),
            node_id: self.node_id.clone(),
            timestamp: SystemTime::now(),
            data: serde_json::json!({
                "term": state.term,
                "health_score": state.health_score,
            }),
        };
        
        for peer in &self.peers {
            tracing::debug!("Sending heartbeat to peer: {}", peer);
            // Implement peer communication logic
        }
        
        Ok(())
    }

    pub async fn monitor_cluster_health(&self) -> Result<(), NexaError> {
        let state = self.state.read().await;
        let now = SystemTime::now();
        
        // Check if we need to trigger leader election due to poor health
        if state.role == NodeRole::Leader && state.health_score < 0.5 {
            info!("Leader health degraded, stepping down");
            drop(state);
            let mut state = self.state.write().await;
            state.role = NodeRole::Follower;
            state.leader_id = None;
            
            let _event = ClusterEvent {
                event_type: "leader_stepdown".to_string(),
                node_id: self.node_id.clone(),
                timestamp: now,
                data: serde_json::json!({
                    "term": state.term,
                    "health_score": state.health_score,
                }),
            };
            
            if let Err(e) = self.event_tx.send(_event) {
                error!("Failed to broadcast event: {}", e);
                return Err(NexaError::system(format!("Broadcast failed: {}", e)));
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    async fn calculate_health_score(&self) -> f64 {
        // TODO: Implement health score calculation in future release
        // This method will calculate a health score based on system metrics
        0.0
    }

    async fn broadcast_event(&self, event: ClusterEvent) -> Result<(), NexaError> {
        if let Err(e) = self.event_tx.send(event) {
            error!("Failed to broadcast event: {}", e);
            return Err(NexaError::system(format!("Broadcast failed: {}", e)));
        }
        Ok(())
    }

    pub async fn handle_event(&self, event: ClusterEvent) -> Result<(), NexaError> {
        match event.event_type.as_str() {
            "heartbeat" => self.handle_heartbeat(event).await,
            "vote_request" => self.handle_vote_request(event).await,
            "vote_response" => self.handle_vote_response(event).await,
            "leader_stepdown" => self.handle_leader_stepdown(event).await,
            _ => {
                warn!("Unknown event type: {}", event.event_type);
                Ok(())
            }
        }
    }

    async fn handle_heartbeat(&self, event: ClusterEvent) -> Result<(), NexaError> {
        let mut state = self.state.write().await;
        let term = event.data["term"].as_u64().unwrap_or(0);
        
        if term >= state.term {
            state.last_heartbeat = event.timestamp;
            state.term = term;
            state.leader_id = Some(event.node_id);
            if state.role != NodeRole::Follower {
                state.role = NodeRole::Follower;
            }
        }
        Ok(())
    }

    async fn handle_vote_request(&self, event: ClusterEvent) -> Result<(), NexaError> {
        let state = self.state.read().await;
        let term = event.data["term"].as_u64().unwrap_or(0);
        
        if term > state.term {
            let response = ClusterEvent {
                event_type: "vote_response".to_string(),
                node_id: state.id.clone(),
                timestamp: std::time::SystemTime::now(),
                data: serde_json::json!({
                    "term": term,
                    "vote_granted": true,
                }),
            };
            self.broadcast_event(response).await?;
        }
        Ok(())
    }

    async fn handle_vote_response(&self, event: ClusterEvent) -> Result<(), NexaError> {
        let mut state = self.state.write().await;
        if state.role == NodeRole::Candidate {
            let vote_granted = event.data["vote_granted"].as_bool().unwrap_or(false);
            if vote_granted {
                // Count votes and become leader if quorum reached
                // TODO: Implement vote counting
                state.role = NodeRole::Leader;
                info!("Node {} became leader for term {}", state.id, state.term);
            }
        }
        Ok(())
    }

    async fn handle_leader_stepdown(&self, event: ClusterEvent) -> Result<(), NexaError> {
        let mut state = self.state.write().await;
        if state.leader_id == Some(event.node_id.clone()) {
            state.leader_id = None;
            state.role = NodeRole::Follower;
            // Trigger new election
            self.check_election_timeout().await?;
        }
        Ok(())
    }
} 