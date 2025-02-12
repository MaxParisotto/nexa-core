use tokio::sync::mpsc;
use log::{debug, error};
use crate::error::NexaError;
use crate::mcp::buffer::MessageBuffer;
use crate::mcp::cluster::ClusterManager;
use crate::mcp::processor::{MessageProcessor, ProcessorConfig};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use uuid::Uuid;

/// Configuration for cluster message processing
#[derive(Debug, Clone)]
pub struct ClusterProcessorConfig {
    /// Local processor configuration
    pub processor_config: ProcessorConfig,
    /// Replication factor for messages
    pub replication_factor: usize,
    /// Message sync interval
    pub sync_interval: Duration,
    /// Message redistribution interval
    pub redistribution_interval: Duration,
}

impl Default for ClusterProcessorConfig {
    fn default() -> Self {
        Self {
            processor_config: ProcessorConfig::default(),
            replication_factor: 2,
            sync_interval: Duration::from_secs(5),
            redistribution_interval: Duration::from_secs(30),
        }
    }
}

/// Tracks message distribution across the cluster
#[derive(Debug)]
struct MessageDistribution {
    /// Maps message IDs to nodes that have a copy
    message_locations: HashMap<Uuid, Vec<Uuid>>,
    /// Maps node IDs to their message counts
    node_message_counts: HashMap<Uuid, usize>,
}

impl MessageDistribution {
    fn new() -> Self {
        Self {
            message_locations: HashMap::new(),
            node_message_counts: HashMap::new(),
        }
    }

    fn add_message(&mut self, msg_id: Uuid, node_id: Uuid) {
        self.message_locations
            .entry(msg_id)
            .or_insert_with(Vec::new)
            .push(node_id);
            
        *self.node_message_counts.entry(node_id).or_insert(0) += 1;
    }

    fn get_nodes_for_message(&self, msg_id: &Uuid) -> Vec<Uuid> {
        self.message_locations
            .get(msg_id)
            .cloned()
            .unwrap_or_default()
    }

    fn get_node_message_count(&self, node_id: &Uuid) -> usize {
        self.node_message_counts.get(node_id).copied().unwrap_or(0)
    }
}

/// Cluster-aware message processor
pub struct ClusterProcessor {
    /// Local message processor
    processor: MessageProcessor,
    /// Cluster manager
    manager: Arc<ClusterManager>,
    /// Message buffer
    _buffer: Arc<MessageBuffer>,
    /// Configuration
    config: ClusterProcessorConfig,
    /// Message distribution tracking
    distribution: Arc<tokio::sync::RwLock<MessageDistribution>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl std::fmt::Debug for ClusterProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClusterProcessor")
            .field("processor", &self.processor)
            .field("config", &self.config)
            .finish()
    }
}

impl ClusterProcessor {
    /// Create a new cluster processor
    pub fn new(
        config: ClusterProcessorConfig,
        buffer: Arc<MessageBuffer>,
        cluster: Arc<ClusterManager>,
    ) -> Self {
        let processor = MessageProcessor::new(config.processor_config.clone(), buffer.clone());
        let distribution = Arc::new(tokio::sync::RwLock::new(MessageDistribution::new()));
        
        Self {
            processor,
            manager: cluster,
            _buffer: buffer,
            config,
            distribution,
            shutdown_tx: None,
        }
    }

    /// Start cluster message processing
    pub async fn start(&mut self) -> Result<(), NexaError> {
        // Start local processor
        self.processor.start().await?;

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Start message sync task
        let sync_task = {
            let _buffer = self._buffer.clone();
            let cluster = self.manager.clone();
            let distribution = self.distribution.clone();
            let config = self.config.clone();
            
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(config.sync_interval);
                loop {
                    interval.tick().await;
                    if let Err(e) = Self::sync_messages(
                        _buffer.clone(),
                        cluster.clone(),
                        distribution.clone(),
                        config.replication_factor,
                    ).await {
                        error!("Message sync failed: {}", e);
                    }
                }
            })
        };

        // Start redistribution task
        let redistribution_task = {
            let _buffer = self._buffer.clone();
            let cluster = self.manager.clone();
            let distribution = self.distribution.clone();
            let config = self.config.clone();
            
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(config.redistribution_interval);
                loop {
                    interval.tick().await;
                    if let Err(e) = Self::redistribute_messages(
                        _buffer.clone(),
                        cluster.clone(),
                        distribution.clone(),
                    ).await {
                        error!("Message redistribution failed: {}", e);
                    }
                }
            })
        };

        // Wait for shutdown signal
        tokio::select! {
            _ = shutdown_rx.recv() => {
                debug!("Shutting down cluster processor");
                sync_task.abort();
                redistribution_task.abort();
            }
        }

        Ok(())
    }

    /// Stop cluster message processing
    pub async fn stop(&mut self) -> Result<(), NexaError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
        self.processor.stop().await
    }

    /// Sync messages across the cluster
    async fn sync_messages(
        buffer: Arc<MessageBuffer>,
        cluster: Arc<ClusterManager>,
        distribution: Arc<tokio::sync::RwLock<MessageDistribution>>,
        replication_factor: usize,
    ) -> Result<(), NexaError> {
        let nodes = cluster.get_active_nodes().await?;
        let mut dist = distribution.write().await;

        // Process all messages in buffer
        while let Some(msg) = buffer.pop_any().await {
            let current_nodes = dist.get_nodes_for_message(&msg.id);
            
            // If message isn't replicated enough, send to more nodes
            if current_nodes.len() < replication_factor {
                let needed = replication_factor - current_nodes.len();
                let available_nodes: Vec<_> = nodes.iter()
                    .filter(|n| !current_nodes.contains(&n.id))
                    .collect();

                for node in available_nodes.iter().take(needed) {
                    if let Err(e) = cluster.send_message_to_node(&msg, node.id).await {
                        error!("Failed to replicate message {} to node {}: {}", msg.id, node.id, e);
                    } else {
                        dist.add_message(msg.id, node.id);
                    }
                }
            }

            // Put message back in buffer
            if let Err(e) = buffer.publish(msg).await {
                error!("Failed to return message to buffer: {}", e);
            }
        }

        Ok(())
    }

    /// Redistribute messages for better balance
    async fn redistribute_messages(
        _buffer: Arc<MessageBuffer>,
        cluster: Arc<ClusterManager>,
        distribution: Arc<tokio::sync::RwLock<MessageDistribution>>,
    ) -> Result<(), NexaError> {
        let nodes = cluster.get_active_nodes().await?;
        let dist = distribution.read().await;

        // Calculate average load
        let total_messages: usize = dist.node_message_counts.values().sum();
        let avg_load = total_messages / nodes.len().max(1);

        // Find overloaded and underloaded nodes
        let mut overloaded: Vec<_> = nodes.iter()
            .filter(|n| dist.get_node_message_count(&n.id) > avg_load + 10)
            .collect();
        let mut underloaded: Vec<_> = nodes.iter()
            .filter(|n| dist.get_node_message_count(&n.id) < avg_load - 10)
            .collect();

        // Balance the load
        while !overloaded.is_empty() && !underloaded.is_empty() {
            let from = overloaded.pop().unwrap();
            let to = underloaded.pop().unwrap();

            if let Err(e) = cluster.transfer_messages(from.id, to.id, 10).await {
                error!("Failed to transfer messages from {} to {}: {}", from.id, to.id, e);
            }
        }

        Ok(())
    }

    /// Rebalance nodes in the cluster to ensure even load distribution
    #[allow(dead_code)]
    async fn rebalance_nodes(&self) -> Result<(), NexaError> {
        // Get node load information
        let nodes = self.manager.get_active_nodes().await?;
        
        if nodes.is_empty() {
            debug!("No nodes to rebalance");
            return Ok(());
        }

        // Calculate load metrics for each node based on capabilities
        let mut node_loads: Vec<(Uuid, f64)> = nodes.iter()
            .map(|node| {
                let cpu_weight = node.capabilities.cpu_cores as f64;
                let memory_weight = (node.capabilities.memory_mb / 1024) as f64; // Convert to GB
                let load = cpu_weight * 0.6 + memory_weight * 0.4;
                (node.id, load)
            })
            .collect();

        // Sort nodes by load
        node_loads.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Calculate average load
        let avg_load = node_loads.iter().map(|(_, load)| load).sum::<f64>() / nodes.len() as f64;
        let threshold = avg_load * 0.2; // 20% deviation threshold

        // Identify overloaded and underloaded nodes
        let overloaded: Vec<_> = node_loads.iter()
            .filter(|(_, load)| *load > avg_load + threshold)
            .collect();
        let underloaded: Vec<_> = node_loads.iter()
            .filter(|(_, load)| *load < avg_load - threshold)
            .collect();

        // Perform load balancing by transferring messages
        for (over_node_id, over_load) in overloaded {
            if let Some((under_node_id, under_load)) = underloaded.first() {
                debug!(
                    "Rebalancing: moving messages from {} (load: {:.2}) to {} (load: {:.2})",
                    over_node_id, over_load, under_node_id, under_load
                );
                
                // Transfer messages in batches
                if let Err(e) = self.manager.transfer_messages(*over_node_id, *under_node_id, 10).await {
                    error!("Failed to transfer messages: {}", e);
                }
            }
        }

        debug!("Rebalancing complete for {} nodes", nodes.len());
        Ok(())
    }
}