use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::{
    monitoring::ResourceMonitor,
    validation::CodeValidator,
    generation::CodeGenerator,
};

pub struct Pipeline {
    generator: Arc<CodeGenerator<'static>>,
    validator: Arc<Mutex<CodeValidator>>,
    monitor: ResourceMonitor,
    max_parallel_jobs: usize,
}

impl Pipeline {
    pub fn new(generator: CodeGenerator<'static>, max_parallel_jobs: usize) -> Self {
        Self {
            generator: Arc::new(generator),
            validator: Arc::new(Mutex::new(CodeValidator::new())),
            monitor: ResourceMonitor::new(),
            max_parallel_jobs,
        }
    }

    pub async fn process_job(&self, job: GenerationJob) -> Result<GenerationOutput, PipelineError> {
        let start = std::time::Instant::now();
        debug!("Starting pipeline job: {:?}", job.id);

        // Monitor resources
        if !self.check_resources().await? {
            return Err(PipelineError::ResourceExhausted);
        }

        // Generate code
        let code = self.generator.generate_code(job.request).await?;

        // Validate output
        let mut validator = self.validator.lock().await;
        validator.validate(&code.code, &job.language)?;

        // Update metrics
        self.monitor.track_token_generation(code.metrics.tokens_generated as u64);

        let duration = start.elapsed();
        debug!("Pipeline job {} completed in {:?}", job.id, duration);

        Ok(GenerationOutput {
            code,
            duration: duration.as_millis() as u64,
        })
    }

    async fn check_resources(&self) -> Result<bool, PipelineError> {
        let metrics = self.monitor.get_metrics_receiver();
        let state = metrics.borrow();

        if state.memory_usage_mb > 90.0 {
            warn!("Memory usage too high: {}MB", state.memory_usage_mb);
            return Ok(false);
        }

        if state.error_rate > 0.1 {
            warn!("Error rate too high: {}", state.error_rate);
            return Ok(false);
        }

        Ok(true)
    }
}

#[derive(Debug)]
pub struct GenerationJob {
    pub id: String,
    pub request: CodeRequest,
    pub language: String,
    pub priority: u8,
}

#[derive(Debug)]
pub struct GenerationOutput {
    pub code: GeneratedCode,
    pub duration: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Resources exhausted")]
    ResourceExhausted,
    #[error("Generation failed: {0}")]
    GenerationFailed(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}
