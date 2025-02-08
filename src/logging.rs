use std::path::PathBuf;
use std::sync::Once;
use std::fs;
use std::sync::Mutex;
use tracing_subscriber::{fmt, EnvFilter, prelude::*};
use std::time::Duration;
use tokio::time;
use tokio::sync::mpsc;

static INIT: Once = Once::new();
const MAX_LOG_FILES: usize = 7; // Keep 7 days of logs
const LOG_ROTATION_INTERVAL: Duration = Duration::from_secs(86400); // 24 hours

// Global channel sender for UI logs
lazy_static::lazy_static! {
    static ref UI_SENDER: Mutex<Option<mpsc::UnboundedSender<String>>> = Mutex::new(None);
}

pub fn set_ui_sender(sender: mpsc::UnboundedSender<String>) {
    let mut ui_sender = UI_SENDER.lock().unwrap();
    *ui_sender = Some(sender);
}

pub fn init(log_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    INIT.call_once(|| {
        // Create log directory if it doesn't exist
        fs::create_dir_all(&log_dir).expect("Failed to create log directory");

        // Set up file appender with daily rotation
        let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("nexa")
            .filename_suffix("log")
            .build(&log_dir)
            .expect("Failed to create file appender");

        // Set up the subscriber with both console and file output
        let subscriber = tracing_subscriber::registry()
            .with(
                fmt::Layer::new()
                    .with_writer(std::io::stdout)
                    .with_file(true)
                    .with_line_number(true)
                    .with_thread_ids(true)
                    .with_target(true)
                    .with_ansi(true)
                    .with_filter(EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new("debug")))
            )
            .with(
                fmt::Layer::new()
                    .with_writer(file_appender)
                    .json()
                    .with_file(true)
                    .with_line_number(true)
                    .with_thread_ids(true)
                    .with_target(true)
                    .with_ansi(false)
                    .with_filter(EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new("debug")))
            );

        // Initialize the subscriber
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set tracing subscriber");

        // Start log rotation task
        let log_dir = log_dir.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(LOG_ROTATION_INTERVAL);
            loop {
                interval.tick().await;
                cleanup_old_logs(&log_dir);
            }
        });
    });
    Ok(())
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