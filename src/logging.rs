use std::path::PathBuf;
use std::sync::Once;
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::sync::Arc;
use tracing_subscriber::{fmt, EnvFilter, prelude::*};
use std::time::Duration;
use tokio::time;

static INIT: Once = Once::new();
const MAX_LOG_FILES: usize = 7; // Keep 7 days of logs
const LOG_ROTATION_INTERVAL: Duration = Duration::from_secs(86400); // 24 hours

pub fn init(log_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    INIT.call_once(|| {
        // Create log directory if it doesn't exist
        fs::create_dir_all(&log_dir).expect("Failed to create log directory");

        // Set up tracing subscriber with custom formatting
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"));

        // Create a file appender layer
        let file_path = get_log_file_path(&log_dir);
        let file_appender = fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(true)
            .with_thread_names(true)
            .with_ansi(false)
            .json()
            .with_writer(Arc::new(OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .expect("Failed to open log file")));

        // Create a console layer
        let console_layer = fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(true)
            .with_ansi(true)
            .compact();

        // Initialize the log compatibility layer first
        if let Ok(()) = tracing_log::LogTracer::init() {
            // Only set up the subscriber if the log tracer was initialized successfully
            if let Err(e) = tracing_subscriber::registry()
                .with(env_filter)
                .with(console_layer)
                .with(file_appender)
                .try_init()
            {
                eprintln!("Failed to initialize tracing subscriber: {}", e);
                return;
            }

            // Start log rotation task
            let log_dir = log_dir.clone();
            tokio::spawn(async move {
                let mut interval = time::interval(LOG_ROTATION_INTERVAL);
                loop {
                    interval.tick().await;
                    cleanup_old_logs(&log_dir);
                }
            });
        } else {
            eprintln!("Failed to initialize log tracer");
        }
    });
    Ok(())
}

fn get_log_file_path(log_dir: &PathBuf) -> PathBuf {
    log_dir.join(format!(
        "nexa_{}.log",
        Local::now().format("%Y%m%d_%H%M%S")
    ))
}

fn cleanup_old_logs(log_dir: &PathBuf) {
    if let Ok(entries) = fs::read_dir(log_dir) {
        let mut log_files: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path()
                    .extension()
                    .map(|ext| ext == "log")
                    .unwrap_or(false)
            })
            .collect();

        // Sort by modification time (newest first)
        log_files.sort_by_key(|entry| {
            std::cmp::Reverse(
                entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or_else(|| std::time::SystemTime::UNIX_EPOCH),
            )
        });

        // Remove old files keeping only MAX_LOG_FILES
        for old_file in log_files.iter().skip(MAX_LOG_FILES) {
            let _ = fs::remove_file(old_file.path());
        }
    }
} 