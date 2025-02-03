use std::collections::VecDeque;
use chrono::{DateTime, Utc};
use sha2::{Sha256, Digest};
use log::warn;

#[derive(Debug, Clone)]
pub struct FrameMetadata {
    pub source: String,
    pub frame_type: FrameType,
    pub importance: u8,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameType {
    Code,
    Conversation,
    SystemPrompt,
    UserQuery,
}

#[derive(Debug)]
pub struct ContextFrame {
    pub timestamp: DateTime<Utc>,
    pub content: String,
    pub metadata: FrameMetadata,
    pub hash: String,
}

impl ContextFrame {
    pub fn new(content: String, metadata: FrameMetadata) -> Self {
        let timestamp = Utc::now();
        let hash = Self::calculate_hash(&content);
        
        Self {
            timestamp,
            content,
            metadata,
            hash,
        }
    }

    fn calculate_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

pub struct EntropyTracker {
    window_size: usize,
    entropy_threshold: f32,
    recent_entropies: VecDeque<f32>,
}

impl EntropyTracker {
    pub fn new(window_size: usize, entropy_threshold: f32) -> Self {
        Self {
            window_size,
            entropy_threshold,
            recent_entropies: VecDeque::with_capacity(window_size),
        }
    }

    pub fn calculate_entropy(&mut self, content: &str) -> f32 {
        let total_chars = content.len() as f32;
        let char_counts: std::collections::HashMap<char, usize> = 
            content.chars().fold(std::collections::HashMap::new(), |mut acc, c| {
                *acc.entry(c).or_insert(0) += 1;
                acc
            });

        let entropy: f32 = char_counts.values()
            .map(|&count| {
                let p = count as f32 / total_chars;
                -p * p.log2()
            })
            .sum();

        self.recent_entropies.push_back(entropy);
        if self.recent_entropies.len() > self.window_size {
            self.recent_entropies.pop_front();
        }

        entropy
    }

    pub fn is_repetitive(&self) -> bool {
        if self.recent_entropies.len() < 2 {
            return false;
        }

        let avg_entropy: f32 = self.recent_entropies.iter().sum::<f32>() / 
            self.recent_entropies.len() as f32;
        avg_entropy < self.entropy_threshold
    }

    pub fn get_average_entropy(&self) -> f32 {
        if self.recent_entropies.is_empty() {
            return 0.0;
        }
        self.recent_entropies.iter().sum::<f32>() / self.recent_entropies.len() as f32
    }
}

pub struct ChangeDetector {
    last_hashes: VecDeque<String>,
    similarity_threshold: f32,
}

impl ChangeDetector {
    pub fn new(max_history: usize, similarity_threshold: f32) -> Self {
        Self {
            last_hashes: VecDeque::with_capacity(max_history),
            similarity_threshold,
        }
    }

    pub fn is_similar(&mut self, new_hash: &str) -> bool {
        self.last_hashes.iter().any(|h| self.calculate_similarity(h, new_hash) > self.similarity_threshold)
    }

    fn calculate_similarity(&self, hash1: &str, hash2: &str) -> f32 {
        let matching_chars = hash1.chars().zip(hash2.chars())
            .filter(|(a, b)| a == b)
            .count();
        matching_chars as f32 / hash1.len() as f32
    }
}

pub struct ContextManager {
    history: VecDeque<ContextFrame>,
    max_frames: usize,
    entropy_tracker: EntropyTracker,
    change_detector: ChangeDetector,
}

impl ContextManager {
    pub fn new(max_frames: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_frames),
            max_frames,
            entropy_tracker: EntropyTracker::new(5, 0.5),
            change_detector: ChangeDetector::new(10, 0.9),
        }
    }

    pub fn add_frame(&mut self, content: String, metadata: FrameMetadata) -> bool {
        let frame = ContextFrame::new(content, metadata);
        
        // Check for repetition and similarity
        if self.entropy_tracker.is_repetitive() {
            warn!("Detected repetitive content pattern");
            return false;
        }

        if self.change_detector.is_similar(&frame.hash) {
            warn!("Content too similar to recent frames");
            return false;
        }

        self.entropy_tracker.calculate_entropy(&frame.content);
        
        if self.history.len() >= self.max_frames {
            self.history.pop_front();
        }
        
        self.history.push_back(frame);
        true
    }

    pub fn build_prompt(&self, task_type: &str, content: &str) -> String {
        let mut prompt = String::new();
        
        // Add relevant context from history
        for frame in self.history.iter().rev().take(3) {
            if frame.metadata.frame_type == FrameType::SystemPrompt {
                prompt.push_str(&frame.content);
                prompt.push_str("\n\n");
            }
        }

        // Add current task context
        prompt.push_str(&format!("Task: {}\n", task_type));
        prompt.push_str(content);
        
        prompt
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn optimize_context(&mut self) -> bool {
        let total_tokens = self.calculate_total_tokens();
        
        if total_tokens > self.max_frames * 3/4 {
            // Remove low importance frames first
            self.history.retain(|frame| frame.metadata.importance > 1);
            return true;
        }
        false
    }

    pub fn calculate_total_tokens(&self) -> usize {
        self.history
            .iter()
            .map(|frame| frame.content.split_whitespace().count())
            .sum()
    }

    pub fn get_context_stats(&self) -> ContextStats {
        ContextStats {
            total_frames: self.history.len(),
            total_tokens: self.calculate_total_tokens(),
            entropy_score: self.entropy_tracker.get_average_entropy(),
            memory_usage: std::mem::size_of_val(&*self) as u64,
        }
    }
}

#[derive(Debug)]
pub struct ContextStats {
    pub total_frames: usize,
    pub total_tokens: usize,
    pub entropy_score: f32,
    pub memory_usage: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_frame_creation() {
        let metadata = FrameMetadata {
            source: "test".to_string(),
            frame_type: FrameType::Code,
            importance: 1,
            tags: vec!["test".to_string()],
        };
        
        let frame = ContextFrame::new("test content".to_string(), metadata);
        assert!(!frame.hash.is_empty());
    }

    #[test]
    fn test_entropy_tracking() {
        let mut tracker = EntropyTracker::new(5, 0.5);
        let entropy = tracker.calculate_entropy("test content");
        assert!(entropy > 0.0);
    }

    #[test]
    fn test_context_optimization() {
        let mut manager = ContextManager::new(10);
        
        // Add some test frames
        for i in 0..12 {
            let metadata = FrameMetadata {
                source: "test".into(),
                frame_type: FrameType::Code,
                importance: (i % 3) as u8,
                tags: vec![],
            };
            manager.add_frame(format!("test content {}", i), metadata);
        }

        let optimized = manager.optimize_context();
        assert!(optimized);
        
        let stats = manager.get_context_stats();
        assert!(stats.total_frames < 12);
    }
}
