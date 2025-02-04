use chrono::{DateTime, Utc};
use super::types::{LogEntry, LogLevel};
use std::collections::VecDeque;

pub const MAX_LOG_ENTRIES: usize = 100;

pub fn add_log_entry(
    logs: &mut VecDeque<LogEntry>,
    message: impl Into<String>,
    level: LogLevel,
    source: impl Into<String>,
) {
    let entry = LogEntry {
        timestamp: Utc::now(),
        message: message.into(),
        level,
        source: source.into(),
    };

    if logs.len() >= MAX_LOG_ENTRIES {
        logs.pop_front();
    }
    logs.push_back(entry);
}

pub fn format_duration(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

pub fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.format("%H:%M:%S%.3f").to_string()
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
} 