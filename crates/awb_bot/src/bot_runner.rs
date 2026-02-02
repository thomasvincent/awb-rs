use crate::checkpoint::Checkpoint;
use crate::config::BotConfig;
use crate::report::{BotReport, PageAction, PageResult};
use awb_domain::types::Title;
use awb_engine::transform::TransformEngine;
use awb_mw_api::client::{EditRequest, MediaWikiClient};
use awb_telemetry::TelemetryEvent;
use chrono::Utc;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use thiserror::Error;
use tokio::signal;

#[derive(Debug, Error)]
pub enum BotError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("Engine error: {0}")]
    EngineError(String),

    #[error("Checkpoint error: {0}")]
    CheckpointError(#[from] crate::checkpoint::CheckpointError),

    #[error("Emergency stop triggered")]
    EmergencyStop,

    #[error("Maximum edits reached: {0}")]
    MaxEditsReached(u32),

    #[error("Maximum runtime exceeded")]
    MaxRuntimeExceeded,

    #[error("Interrupted by signal")]
    Interrupted,
}

/// Bot runner for fully autonomous editing
pub struct BotRunner<C: MediaWikiClient> {
    config: BotConfig,
    client: Arc<C>,
    engine: TransformEngine,
    pages: Vec<String>,
    checkpoint: Checkpoint,
    report: BotReport,
    start_instant: Instant,
}

impl<C: MediaWikiClient> BotRunner<C> {
    /// Create a new bot runner
    pub fn new(config: BotConfig, client: C, engine: TransformEngine, pages: Vec<String>) -> Self {
        let start_time = Utc::now();
        Self {
            config,
            client: Arc::new(client),
            engine,
            pages,
            checkpoint: Checkpoint::new(),
            report: BotReport::new(start_time),
            start_instant: Instant::now(),
        }
    }

    /// Create a bot runner with existing checkpoint
    pub fn with_checkpoint(
        config: BotConfig,
        client: C,
        engine: TransformEngine,
        pages: Vec<String>,
        checkpoint: Checkpoint,
    ) -> Self {
        let start_time = Utc::now();
        Self {
            config,
            client: Arc::new(client),
            engine,
            pages,
            checkpoint,
            report: BotReport::new(start_time),
            start_instant: Instant::now(),
        }
    }

    /// Run the bot
    pub async fn run(&mut self) -> Result<BotReport, BotError> {
        tracing::info!("Starting bot run with {} pages", self.pages.len());
        self.emit_telemetry(TelemetryEvent::session_started("bot"));

        // Setup signal handler for graceful shutdown
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shutdown_flag_clone = shutdown_flag.clone();
        tokio::spawn(async move {
            if let Ok(()) = signal::ctrl_c().await {
                tracing::info!("Received interrupt signal");
                shutdown_flag_clone.store(true, Ordering::SeqCst);
            }
        });

        let start_index = self.checkpoint.next_index();
        for (index, page_title) in self.pages.iter().enumerate().skip(start_index) {
            // Check stop conditions
            if let Some(reason) = self.should_stop()? {
                tracing::info!("Stopping bot: {}", reason);
                self.report.finalize(false, Some(reason));
                return Ok(self.report.clone());
            }

            // Check for interrupt
            if shutdown_flag.load(Ordering::SeqCst) {
                tracing::info!("Graceful shutdown initiated");
                self.report
                    .finalize(false, Some("Interrupted by user".to_string()));
                return Err(BotError::Interrupted);
            }

            // Process page
            match self.process_page(page_title).await {
                Ok(result) => {
                    self.report.record_page(result.clone());
                    let (edited, skipped, errored) = match result.action {
                        PageAction::Edited => (true, false, false),
                        PageAction::Skipped => (false, true, false),
                        PageAction::Errored => (false, false, true),
                    };
                    self.checkpoint
                        .record_page(page_title.clone(), edited, skipped, errored);
                }
                Err(e) => {
                    tracing::error!("Error processing page {}: {}", page_title, e);
                    let result = PageResult {
                        title: page_title.clone(),
                        action: PageAction::Errored,
                        diff_summary: None,
                        warnings: vec![],
                        error: Some(e.to_string()),
                        timestamp: Utc::now(),
                    };
                    self.report.record_page(result);
                    self.checkpoint
                        .record_page(page_title.clone(), false, false, true);
                }
            }

            // Log progress
            if (index + 1) % self.config.log_every_n as usize == 0 {
                tracing::info!(
                    "Progress: {}/{} pages ({} edited, {} skipped, {} errors)",
                    index + 1,
                    self.pages.len(),
                    self.report.pages_edited,
                    self.report.pages_skipped,
                    self.report.pages_errored
                );
            }
        }

        tracing::info!("Bot run completed successfully");
        self.report
            .finalize(true, Some("All pages processed".to_string()));
        self.emit_telemetry(TelemetryEvent::session_completed(
            self.report.pages_processed,
            self.report.pages_edited,
            self.report.pages_skipped,
            self.report.pages_errored,
            self.report.elapsed_secs,
        ));

        Ok(self.report.clone())
    }

    /// Process a single page
    async fn process_page(&self, page_title: &str) -> Result<PageResult, BotError> {
        let page_start = Instant::now();
        tracing::debug!("Processing page: {}", page_title);

        // Parse title (simplified - in production would use proper parsing)
        let title = Title::new(awb_domain::types::Namespace::MAIN, page_title);

        // Fetch page content
        let page = self
            .client
            .get_page(&title)
            .await
            .map_err(|e| BotError::ApiError(e.to_string()))?;

        // Apply transformations
        let plan = self.engine.apply(&page);

        // Check for no changes
        if plan.new_wikitext == page.wikitext {
            if self.config.skip_no_change {
                tracing::debug!("Skipping page {} (no changes)", page_title);
                return Ok(PageResult {
                    title: page_title.to_string(),
                    action: PageAction::Skipped,
                    diff_summary: Some("No changes needed".to_string()),
                    warnings: vec![],
                    error: None,
                    timestamp: Utc::now(),
                });
            }
        }

        // Check for warnings
        let warnings: Vec<String> = plan.warnings.iter().map(|w| format!("{:?}", w)).collect();

        if !warnings.is_empty() && self.config.skip_on_warning {
            tracing::debug!("Skipping page {} (warnings present)", page_title);
            return Ok(PageResult {
                title: page_title.to_string(),
                action: PageAction::Skipped,
                diff_summary: Some("Skipped due to warnings".to_string()),
                warnings: warnings.clone(),
                error: None,
                timestamp: Utc::now(),
            });
        }

        // Emit warnings as telemetry
        for warning in &plan.warnings {
            self.emit_telemetry(TelemetryEvent::Warning {
                message: format!("Page {}: {:?}", page_title, warning),
                timestamp: Utc::now(),
            });
        }

        // Save edit (unless dry-run)
        if !self.config.dry_run {
            let edit_request = EditRequest {
                title: title.clone(),
                text: plan.new_wikitext.clone(),
                summary: plan.summary.clone(),
                minor: true,
                bot: true,
                base_timestamp: page.timestamp.to_rfc3339(),
                start_timestamp: Utc::now().to_rfc3339(),
                section: None,
            };

            let response = self
                .client
                .edit_page(&edit_request)
                .await
                .map_err(|e| BotError::ApiError(e.to_string()))?;

            if response.result != "Success" {
                return Err(BotError::ApiError(format!(
                    "Edit failed for {}: {}",
                    page_title, response.result
                )));
            }

            // Warn if MediaWiki returned "Success" without creating a new revision
            if response.new_revid.is_none() {
                tracing::warn!(
                    "Page {} returned Success but no new_revid - edit may not have been saved",
                    page_title
                );
            }

            let duration = page_start.elapsed().as_millis() as u64;
            self.emit_telemetry(TelemetryEvent::PageProcessed {
                title: page_title.to_string(),
                outcome: "edited".to_string(),
                duration_ms: duration,
                timestamp: Utc::now(),
            });

            tracing::info!("Saved page {} (rev: {:?})", page_title, response.new_revid);

            Ok(PageResult {
                title: page_title.to_string(),
                action: PageAction::Edited,
                diff_summary: Some(format!("{} rules applied", plan.rules_applied.len())),
                warnings,
                error: None,
                timestamp: Utc::now(),
            })
        } else {
            tracing::info!("Dry-run: would edit page {}", page_title);
            Ok(PageResult {
                title: page_title.to_string(),
                action: PageAction::Skipped,
                diff_summary: Some(format!(
                    "Dry-run: {} rules would apply",
                    plan.rules_applied.len()
                )),
                warnings,
                error: None,
                timestamp: Utc::now(),
            })
        }
    }

    /// Check if bot should stop
    fn should_stop(&self) -> Result<Option<String>, BotError> {
        // Check emergency stop file
        if self.config.emergency_stop_file.exists() {
            return Err(BotError::EmergencyStop);
        }

        // Check max edits
        if let Some(max) = self.config.max_edits {
            if self.report.pages_edited >= max as usize {
                return Ok(Some(format!("Maximum edits reached: {}", max)));
            }
        }

        // Check max runtime
        if let Some(max_duration) = self.config.max_runtime {
            let elapsed = self.start_instant.elapsed();
            if elapsed >= max_duration {
                return Ok(Some("Maximum runtime exceeded".to_string()));
            }
        }

        Ok(None)
    }

    /// Emit telemetry event
    fn emit_telemetry(&self, event: TelemetryEvent) {
        // In production, this would use the telemetry system
        tracing::trace!("Telemetry: {:?}", event);
    }

    /// Save checkpoint to file
    pub fn save_checkpoint(&self, path: &Path) -> Result<(), BotError> {
        self.checkpoint.save(path)?;
        tracing::info!("Checkpoint saved to {}", path.display());
        Ok(())
    }

    /// Get current report
    pub fn report(&self) -> &BotReport {
        &self.report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use awb_domain::rules::RuleSet;
    use awb_domain::types::{
        Namespace, PageContent, PageId, PageProperties, ProtectionInfo, RevisionId,
    };
    use awb_engine::general_fixes::FixRegistry;
    use awb_mw_api::client::EditResponse;
    use awb_mw_api::error::MwApiError;
    use awb_mw_api::oauth::{OAuth1Config, OAuthSession};
    use std::collections::HashSet;

    // Mock MediaWiki client for testing
    struct MockClient {
        pages: std::collections::HashMap<String, PageContent>,
    }

    impl MockClient {
        fn new() -> Self {
            Self {
                pages: std::collections::HashMap::new(),
            }
        }

        fn add_page(&mut self, title: &str, wikitext: &str) {
            let page = PageContent {
                page_id: PageId(1),
                title: Title::new(Namespace::MAIN, title),
                revision: RevisionId(100),
                timestamp: Utc::now(),
                wikitext: wikitext.to_string(),
                size_bytes: wikitext.len() as u64,
                is_redirect: false,
                protection: ProtectionInfo::default(),
                properties: PageProperties::default(),
            };
            self.pages.insert(title.to_string(), page);
        }
    }

    #[async_trait]
    impl MediaWikiClient for MockClient {
        async fn login_bot_password(
            &self,
            _username: &str,
            _password: &str,
        ) -> Result<(), MwApiError> {
            Ok(())
        }

        async fn login_oauth1(&self, _config: OAuth1Config) -> Result<(), MwApiError> {
            Ok(())
        }

        async fn login_oauth2(&self, _session: OAuthSession) -> Result<(), MwApiError> {
            Ok(())
        }

        async fn fetch_csrf_token(&self) -> Result<String, MwApiError> {
            Ok("mocktoken".to_string())
        }

        async fn get_page(&self, title: &Title) -> Result<PageContent, MwApiError> {
            self.pages
                .get(&title.display)
                .cloned()
                .ok_or_else(|| MwApiError::ApiError {
                    code: "notfound".to_string(),
                    info: "Page not found".to_string(),
                })
        }

        async fn edit_page(&self, _edit: &EditRequest) -> Result<EditResponse, MwApiError> {
            Ok(EditResponse {
                result: "Success".to_string(),
                new_revid: Some(101),
                new_timestamp: Some(Utc::now().to_rfc3339()),
            })
        }

        async fn parse_wikitext(
            &self,
            _wikitext: &str,
            _title: &Title,
        ) -> Result<String, MwApiError> {
            Ok("<html>parsed</html>".to_string())
        }
    }

    #[tokio::test]
    async fn test_bot_runner_new() {
        let config = BotConfig::default();
        let client = MockClient::new();
        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();
        let pages = vec!["Page1".to_string()];

        let runner = BotRunner::new(config, client, engine, pages);
        assert_eq!(runner.pages.len(), 1);
    }

    #[tokio::test]
    async fn test_bot_runner_skip_no_change() {
        let config = BotConfig::default().with_skip_no_change(true);
        let mut client = MockClient::new();
        client.add_page("TestPage", "unchanged content");

        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();
        let pages = vec!["TestPage".to_string()];

        let runner = BotRunner::new(config, client, engine, pages);
        let result = runner.process_page("TestPage").await.unwrap();

        assert_eq!(result.action, PageAction::Skipped);
    }

    #[tokio::test]
    async fn test_bot_runner_dry_run() {
        let config = BotConfig::default().with_dry_run(true);
        let mut client = MockClient::new();
        client.add_page("TestPage", "test content");

        let mut ruleset = RuleSet::new();
        ruleset.add(awb_domain::rules::Rule::new_plain("test", "modified", true));

        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();
        let pages = vec!["TestPage".to_string()];

        let runner = BotRunner::new(config, client, engine, pages);
        let result = runner.process_page("TestPage").await.unwrap();

        // In dry-run mode, pages with changes are still "skipped" (not actually saved)
        assert_eq!(result.action, PageAction::Skipped);
        assert!(result.diff_summary.unwrap().contains("Dry-run"));
    }
}
