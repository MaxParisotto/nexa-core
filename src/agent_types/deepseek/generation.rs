use std::time::Duration;
use tokio::time::timeout;
use log::{debug, info, warn};

use crate::{
    context::ContextManager,
    errors::{GenerationError, Result},
    llm::LlamaModel,
    validation::{CodeValidator, TestValidator},
};

#[derive(Debug)]
pub struct CodeRequest {
    pub prompt: String,
    pub language: String,
    pub max_tokens: usize,
    pub temperature: f32,
    pub test_required: bool,
    pub review_required: bool,
}

#[derive(Debug)]
pub struct GeneratedCode {
    pub code: String,
    pub tests: Option<String>,
    pub review: Option<String>,
    pub metrics: GenerationMetrics,
}

#[derive(Debug, Default)]
pub struct GenerationMetrics {
    pub generation_time_ms: u64,
    pub tokens_generated: usize,
    pub required_attempts: u8,
    pub validation_passed: bool,
}

const MAX_RETRIES: u8 = 3;
const GENERATION_TIMEOUT: Duration = Duration::from_secs(30);

pub struct CodeGenerator<'a> {
    model: &'a LlamaModel,
    context: &'a ContextManager,
    validator: CodeValidator,
    test_validator: TestValidator,
    resource_monitor: ResourceMonitor,
    intervention_manager: InterventionManager,
}

impl<'a> CodeGenerator<'a> {
    pub fn new(
        model: &'a LlamaModel,
        context: &'a ContextManager,
    ) -> Self {
        let resource_monitor = ResourceMonitor::new();
        let loop_detector = LoopDetector::new(5, 0.8, 3, resource_monitor.clone());
        let intervention_manager = InterventionManager::new(
            resource_monitor.clone(),
            loop_detector,
            MAX_RETRIES,
        );

        Self {
            model,
            context,
            validator: CodeValidator::new(),
            test_validator: TestValidator::new(),
            resource_monitor,
            intervention_manager,
        }
    }

    pub async fn generate_code(
        &self,
        request: CodeRequest,
    ) -> Result<GeneratedCode> {
        let start_time = std::time::Instant::now();
        let mut metrics = GenerationMetrics::default();

        // 1. Validate and prepare context
        debug!("Preparing context for code generation");
        let enhanced_prompt = self.prepare_context(&request)?;

        // Track token usage during generation
        self.resource_monitor.track_token_generation(
            metrics.tokens_generated as u64
        );

        // 2. Generate with fallbacks
        let code = self.intervention_manager
            .check_and_intervene(|| async {
                self.generate_with_fallback(&enhanced_prompt, &request, &mut metrics).await
            })
            .await?;

        // 3. Validate output
        self.validator.validate(&code, &request.language)?;
        metrics.validation_passed = true;

        // 4. Generate tests if required
        let tests = if request.test_required {
            Some(self.generate_tests(&code, &request).await?)
        } else {
            None
        };

        // 5. Review own code if required
        let review = if request.review_required {
            Some(self.review_code(&code, &tests, &request).await?)
        } else {
            None
        };

        metrics.generation_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(GeneratedCode {
            code,
            tests,
            review,
            metrics,
        })
    }

    async fn generate_with_fallback(
        &self,
        prompt: &str,
        request: &CodeRequest,
        metrics: &mut GenerationMetrics,
    ) -> Result<String> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < MAX_RETRIES {
            attempts += 1;
            metrics.required_attempts = attempts;

            let temperature = request.temperature * (1.0 + (attempts as f32 * 0.1));
            
            match timeout(
                GENERATION_TIMEOUT,
                self.model.generate(prompt, request.max_tokens, temperature)
            ).await {
                Ok(Ok(code)) => {
                    if self.validator.quick_validate(&code) {
                        return Ok(code);
                    }
                }
                Ok(Err(e)) => last_error = Some(e),
                Err(_) => {
                    warn!("Generation timeout on attempt {}", attempts);
                    continue;
                }
            }

            // Track tokens processed
            self.resource_monitor.track_token_processing(
                metrics.tokens_generated as u64
            );
        }

        Err(last_error.unwrap_or_else(|| GenerationError::MaxRetriesExceeded))
    }

    async fn generate_tests(&self, code: &str, request: &CodeRequest) -> Result<String> {
        let test_prompt = self.prepare_test_prompt(code, &request.language);
        let tests = self.model
            .generate(&test_prompt, request.max_tokens, request.temperature)
            .await?;

        self.test_validator.validate(&tests, code)?;
        Ok(tests)
    }

    async fn review_code(
        &self,
        code: &str,
        tests: &Option<String>,
        request: &CodeRequest,
    ) -> Result<String> {
        let review_prompt = self.prepare_review_prompt(code, tests, &request.language);
        self.model
            .generate(&review_prompt, request.max_tokens / 2, request.temperature)
            .await
    }

    fn prepare_context(&self, request: &CodeRequest) -> Result<String> {
        let mut context = format!(
            "Language: {}\nTask: Generate code according to the following requirements:\n\n",
            request.language
        );
        context.push_str(&request.prompt);
        Ok(context)
    }

    fn prepare_test_prompt(&self, code: &str, language: &str) -> String {
        format!(
            "Generate comprehensive tests for the following {} code:\n\n{}\n\nTests:",
            language, code
        )
    }

    fn prepare_review_prompt(&self, code: &str, tests: &Option<String>, language: &str) -> String {
        let mut prompt = format!(
            "Review the following {} code for best practices, potential issues, and improvements:\n\n{}",
            language, code
        );

        if let Some(test_code) = tests {
            prompt.push_str("\n\nAssociated tests:\n\n");
            prompt.push_str(test_code);
        }

        prompt.push_str("\n\nCode Review:");
        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_code_generation_pipeline() {
        // Test implementation here
    }
}
