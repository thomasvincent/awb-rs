use crate::checkpoint::Checkpoint;
use crate::config::BotConfig;
use crate::report::{BotReport, PageAction, PageResult};
use awb_domain::types::Title;
use awb_engine::transform::TransformEngine;
use awb_mw_api::client::{EditRequest, MediaWikiClient};
use awb_mw_api::error::MwApiError;
use awb_security::redact_secrets;
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
    secrets: Vec<String>,
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
            secrets: Vec::new(),
        }
    }

    /// Add a secret to be redacted from error messages
    pub fn add_secret(&mut self, secret: String) {
        self.secrets.push(secret);
    }

    /// Redact known secrets from an error message
    fn redact_error_message(&self, message: &str) -> String {
        let secret_refs: Vec<&str> = self.secrets.iter().map(|s| s.as_str()).collect();
        redact_secrets(message, &secret_refs)
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
            secrets: Vec::new(),
        }
    }

    /// Run the bot
    #[tracing::instrument(skip(self), fields(
        total_pages = self.pages.len(),
        bot_name = %self.config.bot_name
    ))]
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

        let mut pages_since_save: u32 = 0;

        for (index, page_title) in self.pages.iter().enumerate() {
            // Identity-based resume: skip pages already completed in a previous run.
            // This is safe even if the page list is reordered between runs.
            if self.checkpoint.is_completed(page_title) {
                continue;
            }
            // Check stop conditions
            if let Some(reason) = self.should_stop()? {
                tracing::info!("Stopping bot: {}", reason);
                self.persist_checkpoint().await;
                self.report.finalize(false, Some(reason));
                return Ok(self.report.clone());
            }

            // Check for interrupt
            if shutdown_flag.load(Ordering::SeqCst) {
                tracing::info!("Graceful shutdown initiated");
                self.persist_checkpoint().await;
                self.report
                    .finalize(false, Some("Interrupted by user".to_string()));
                return Err(BotError::Interrupted);
            }

            // Process page
            let page_span = tracing::info_span!(
                "process_page",
                page_title = %page_title,
                namespace = tracing::field::Empty
            );
            match self.process_page_instrumented(page_title, page_span).await {
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
                    let error_msg = e.to_string();
                    let redacted_msg = self.redact_error_message(&error_msg);
                    tracing::error!("Error processing page {}: {}", page_title, redacted_msg);
                    let result = PageResult {
                        title: page_title.clone(),
                        action: PageAction::Errored,
                        diff_summary: None,
                        warnings: vec![],
                        error: Some(redacted_msg),
                        timestamp: Utc::now(),
                    };
                    self.report.record_page(result);
                    self.checkpoint
                        .record_page(page_title.clone(), false, false, true);
                }
            }

            // Periodic checkpoint persistence (every save_every_n pages)
            pages_since_save += 1;
            if pages_since_save >= self.config.save_every_n {
                self.persist_checkpoint().await;
                pages_since_save = 0;
            }

            // Log progress
            if self.config.log_every_n > 0 && (index + 1) % self.config.log_every_n as usize == 0 {
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
        self.persist_checkpoint().await;
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

    /// Process a single page with instrumentation
    async fn process_page_instrumented(
        &self,
        page_title: &str,
        span: tracing::Span,
    ) -> Result<PageResult, BotError> {
        let _guard = span.enter();
        self.process_page(page_title).await
    }

    /// Process a single page
    async fn process_page(&self, page_title: &str) -> Result<PageResult, BotError> {
        let page_start = Instant::now();
        tracing::debug!("Processing page: {}", page_title);

        // Parse title using namespace_util for proper namespace detection
        let parsed = awb_engine::namespace_util::parse_title(page_title);

        // Record namespace in current span
        tracing::Span::current().record("namespace", format!("{:?}", parsed.namespace));

        // Enforce namespace policy
        if !self.config.is_namespace_allowed(parsed.namespace) {
            tracing::debug!(
                "Skipping page {} (namespace {:?} not allowed)",
                page_title,
                parsed.namespace
            );
            return Ok(PageResult {
                title: page_title.to_string(),
                action: PageAction::Skipped,
                diff_summary: Some(format!(
                    "Namespace {:?} not in allowed list",
                    parsed.namespace
                )),
                warnings: vec![],
                error: None,
                timestamp: Utc::now(),
            });
        }

        let title = Title::new(parsed.namespace, &parsed.name);

        // Fetch page content
        let page = self
            .client
            .get_page(&title)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                let redacted = self.redact_error_message(&msg);
                BotError::ApiError(redacted)
            })?;

        // Check {{bots}}/{{nobots}} policy before transforming
        let policy_result =
            awb_engine::bot_policy::check_bot_allowed(&page.wikitext, &self.config.bot_name);
        if !policy_result.is_allowed() {
            let reason = match &policy_result {
                awb_engine::bot_policy::BotPolicyResult::Denied { reason } => reason.clone(),
                _ => "unknown".to_string(),
            };
            tracing::info!("Skipping page {} (bot policy: {})", page_title, reason);
            return Ok(PageResult {
                title: page_title.to_string(),
                action: PageAction::Skipped,
                diff_summary: Some(format!("Bot policy denied: {}", reason)),
                warnings: vec![],
                error: None,
                timestamp: Utc::now(),
            });
        }

        // Apply transformations
        let plan = self.engine.apply(&page);

        // Check for no changes
        if plan.new_wikitext == page.wikitext && self.config.skip_no_change {
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

        // WP:COSMETIC: skip edits that are cosmetic-only in unattended mode
        if plan.is_cosmetic_only && self.config.skip_cosmetic_only {
            tracing::debug!(
                "Skipping page {} (cosmetic-only edit, WP:COSMETIC)",
                page_title
            );
            return Ok(PageResult {
                title: page_title.to_string(),
                action: PageAction::Skipped,
                diff_summary: Some("Cosmetic-only edit skipped (WP:COSMETIC)".to_string()),
                warnings: vec![],
                error: None,
                timestamp: Utc::now(),
            });
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
            let edit_span = tracing::info_span!(
                "edit_operation",
                action = tracing::field::Empty,
                rules_applied = plan.rules_applied.len()
            );
            let _edit_guard = edit_span.enter();

            // Retry loop for edit conflicts (max 2 attempts)
            let max_retries = 1; // 1 retry = 2 total attempts
            let mut attempt = 0;

            loop {
                // Fetch latest page content if this is a retry
                let current_page = if attempt > 0 {
                    tracing::debug!("Retrying edit for {} (attempt {})", page_title, attempt + 1);
                    self.client.get_page(&title).await.map_err(|e| {
                        let msg = e.to_string();
                        let redacted = self.redact_error_message(&msg);
                        BotError::ApiError(redacted)
                    })?
                } else {
                    page.clone()
                };

                // Re-apply transformations if this is a retry (page may have changed)
                let current_plan = if attempt > 0 {
                    self.engine.apply(&current_page)
                } else {
                    plan.clone()
                };

                let edit_request = EditRequest {
                    title: title.clone(),
                    text: current_plan.new_wikitext.clone(),
                    summary: current_plan.summary.clone(),
                    minor: true,
                    bot: true,
                    base_timestamp: current_page.timestamp.to_rfc3339(),
                    start_timestamp: Utc::now().to_rfc3339(),
                    section: None,
                };

                let response = self.client.edit_page(&edit_request).await;

                match response {
                    Ok(resp) => {
                        if resp.result != "Success" {
                            tracing::Span::current().record("action", "failed");
                            return Err(BotError::ApiError(format!(
                                "Edit failed for {}: {}",
                                page_title, resp.result
                            )));
                        }

                        // Success - break out of retry loop
                        tracing::Span::current().record("action", "edit");
                        if attempt > 0 {
                            tracing::info!("Edit conflict resolved after retry for {}", page_title);
                        }

                        // Warn if MediaWiki returned "Success" without creating a new revision
                        if resp.new_revid.is_none() {
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

                        tracing::info!("Saved page {} (rev: {:?})", page_title, resp.new_revid);

                        // Sleep after successful edit to respect rate limits
                        tokio::time::sleep(self.config.edit_delay).await;

                        return Ok(PageResult {
                            title: page_title.to_string(),
                            action: PageAction::Edited,
                            diff_summary: Some(format!("{} rules applied", current_plan.rules_applied.len())),
                            warnings,
                            error: None,
                            timestamp: Utc::now(),
                        });
                    }
                    Err(MwApiError::EditConflict { base_rev, current_rev }) => {
                        if attempt >= max_retries {
                            // Max retries exceeded - skip this page
                            tracing::Span::current().record("action", "skip");
                            tracing::warn!(
                                "Edit conflict persisted after {} attempts for {}: base={:?}, current={:?}",
                                attempt + 1, page_title, base_rev, current_rev
                            );
                            return Ok(PageResult {
                                title: page_title.to_string(),
                                action: PageAction::Skipped,
                                diff_summary: Some("Edit conflict persisted after retry".to_string()),
                                warnings,
                                error: None,
                                timestamp: Utc::now(),
                            });
                        }

                        // Retry
                        tracing::debug!(
                            "Edit conflict for {}: base={:?}, current={:?}",
                            page_title, base_rev, current_rev
                        );
                        attempt += 1;
                        continue;
                    }
                    Err(e) => {
                        // Other errors - fail immediately
                        let msg = e.to_string();
                        let redacted = self.redact_error_message(&msg);
                        return Err(BotError::ApiError(redacted));
                    }
                }
            }
        } else {
            let dry_run_span = tracing::info_span!(
                "edit_operation",
                action = "skip",
                rules_applied = plan.rules_applied.len()
            );
            let _dry_run_guard = dry_run_span.enter();

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

    /// Persist checkpoint to disk using spawn_blocking to avoid blocking the async runtime.
    /// Logs errors but does not fail the run — checkpoint loss is bounded by save_every_n.
    async fn persist_checkpoint(&self) {
        if let Some(ref cp_path) = self.config.checkpoint_path {
            let checkpoint_data = self.checkpoint.clone();
            let path = cp_path.clone();
            let result = tokio::task::spawn_blocking(move || checkpoint_data.save(&path)).await;
            match result {
                Ok(Ok(())) => tracing::debug!("Checkpoint saved"),
                Ok(Err(e)) => tracing::error!("Failed to save checkpoint: {}", e),
                Err(e) => tracing::error!("Checkpoint save task panicked: {}", e),
            }
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
    use std::time::Duration;

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
    async fn test_bot_runner_nobots_skips_page() {
        let config = BotConfig::default().with_bot_name("TestBot");
        let mut client = MockClient::new();
        client.add_page("NobotPage", "Some text\n{{nobots}}\nMore text");

        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let runner = BotRunner::new(config, client, engine, vec!["NobotPage".to_string()]);
        let result = runner.process_page("NobotPage").await.unwrap();

        assert_eq!(result.action, PageAction::Skipped);
        assert!(result.diff_summary.unwrap().contains("Bot policy denied"));
    }

    #[tokio::test]
    async fn test_bot_runner_bots_deny_specific() {
        let config = BotConfig::default().with_bot_name("AWB-RS");
        let mut client = MockClient::new();
        client.add_page("DenyPage", "Text {{bots|deny=AWB-RS}} more");

        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let runner = BotRunner::new(config, client, engine, vec!["DenyPage".to_string()]);
        let result = runner.process_page("DenyPage").await.unwrap();

        assert_eq!(result.action, PageAction::Skipped);
    }

    #[tokio::test]
    async fn test_bot_runner_namespace_enforcement() {
        // Default config only allows MAIN namespace
        let config = BotConfig::default();
        let client = MockClient::new();

        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let runner = BotRunner::new(
            config,
            client,
            engine,
            vec!["Talk:SomePage".to_string()],
        );
        let result = runner.process_page("Talk:SomePage").await.unwrap();

        assert_eq!(result.action, PageAction::Skipped);
        assert!(result.diff_summary.unwrap().contains("Namespace"));
    }

    #[tokio::test]
    async fn test_bot_runner_namespace_main_allowed() {
        let config = BotConfig::default();
        let mut client = MockClient::new();
        client.add_page("MainPage", "unchanged content");

        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let runner = BotRunner::new(config, client, engine, vec!["MainPage".to_string()]);
        let result = runner.process_page("MainPage").await.unwrap();

        // Should proceed (not skipped for namespace), but skipped for no-change
        assert_eq!(result.action, PageAction::Skipped);
        assert!(result.diff_summary.unwrap().contains("No changes"));
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

    #[tokio::test]
    async fn test_identity_based_resume_skips_completed() {
        // Simulate a checkpoint where "PageA" was already completed
        let mut checkpoint = Checkpoint::new();
        checkpoint.record_page("PageA".to_string(), true, false, false);

        let config = BotConfig::default().with_skip_no_change(true);
        let mut client = MockClient::new();
        client.add_page("PageA", "content A");
        client.add_page("PageB", "content B");

        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        // Pages list is reordered: PageB first, PageA second
        let pages = vec!["PageB".to_string(), "PageA".to_string()];
        let mut runner = BotRunner::with_checkpoint(config, client, engine, pages, checkpoint);
        let report = runner.run().await.unwrap();

        // PageA should be skipped (already completed), PageB processed (skipped for no-change)
        assert_eq!(report.pages_processed, 1); // only PageB
        assert!(!runner.checkpoint.is_completed("PageC")); // sanity
        assert!(runner.checkpoint.is_completed("PageA")); // from previous run
        assert!(runner.checkpoint.is_completed("PageB")); // newly processed
    }

    #[tokio::test]
    async fn test_namespace_image_alias_skipped() {
        // "Image:" is an alias for File namespace, which is not in the default allowlist
        let config = BotConfig::default();
        let client = MockClient::new();

        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let runner = BotRunner::new(config, client, engine, vec!["Image:Foo.jpg".to_string()]);
        let result = runner.process_page("Image:Foo.jpg").await.unwrap();

        assert_eq!(result.action, PageAction::Skipped);
        assert!(result.diff_summary.unwrap().contains("Namespace"));
    }

    #[tokio::test]
    async fn test_namespace_user_skipped() {
        let config = BotConfig::default();
        let client = MockClient::new();

        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let runner =
            BotRunner::new(config, client, engine, vec!["User:Example".to_string()]);
        let result = runner.process_page("User:Example").await.unwrap();

        assert_eq!(result.action, PageAction::Skipped);
        assert!(result.diff_summary.unwrap().contains("Namespace"));
    }

    #[tokio::test]
    async fn test_secret_redaction_in_error_messages() {
        let config = BotConfig::default();
        let client = MockClient::new();

        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let mut runner = BotRunner::new(config, client, engine, vec!["NonexistentPage".to_string()]);

        // Add a secret that might appear in API errors
        runner.add_secret("mysecret123456".to_string());

        // Process a page that doesn't exist to trigger an error
        let result = runner.process_page("NonexistentPage").await;

        // The error should occur but not contain the raw secret
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();

        // Verify the error message doesn't contain the secret
        assert!(!error_msg.contains("mysecret123456"), "Secret should be redacted from error message");
    }

    #[tokio::test]
    async fn test_secret_redaction_in_page_result() {
        // Mock client that returns API errors containing secrets
        struct SecretLeakingClient;

        #[async_trait]
        impl MediaWikiClient for SecretLeakingClient {
            async fn login_bot_password(&self, _username: &str, _password: &str) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn login_oauth1(&self, _config: OAuth1Config) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn login_oauth2(&self, _session: OAuthSession) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn fetch_csrf_token(&self) -> Result<String, MwApiError> {
                Ok("token".to_string())
            }

            async fn get_page(&self, _title: &Title) -> Result<PageContent, MwApiError> {
                // Return error containing a secret
                Err(MwApiError::ApiError {
                    code: "auth_error".to_string(),
                    info: "Authentication failed with token=secret987654321".to_string(),
                })
            }

            async fn edit_page(&self, _edit: &EditRequest) -> Result<EditResponse, MwApiError> {
                Ok(EditResponse {
                    result: "Success".to_string(),
                    new_revid: Some(1),
                    new_timestamp: Some(Utc::now().to_rfc3339()),
                })
            }

            async fn parse_wikitext(&self, _wikitext: &str, _title: &Title) -> Result<String, MwApiError> {
                Ok("<html></html>".to_string())
            }
        }

        let config = BotConfig::default();
        let client = SecretLeakingClient;
        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let mut runner = BotRunner::new(config, client, engine, vec!["TestPage".to_string()]);
        runner.add_secret("secret987654321".to_string());

        let result = runner.process_page("TestPage").await;

        // Should fail due to API error
        assert!(result.is_err());
        let err = result.unwrap_err();
        let error_msg = err.to_string();

        // Verify secret is redacted
        assert!(!error_msg.contains("secret987654321"), "Secret should be redacted from error message");
        assert!(error_msg.contains("[REDACTED]"), "Redacted placeholder should be present");
    }

    #[tokio::test]
    async fn test_secret_redaction_end_to_end_in_report() {
        // Mock client that leaks secrets in errors
        struct SecretLeakingClient;

        #[async_trait]
        impl MediaWikiClient for SecretLeakingClient {
            async fn login_bot_password(&self, _username: &str, _password: &str) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn login_oauth1(&self, _config: OAuth1Config) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn login_oauth2(&self, _session: OAuthSession) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn fetch_csrf_token(&self) -> Result<String, MwApiError> {
                Ok("token".to_string())
            }

            async fn get_page(&self, _title: &Title) -> Result<PageContent, MwApiError> {
                Err(MwApiError::ApiError {
                    code: "forbidden".to_string(),
                    info: "Access denied for user with password=mypassword12345678".to_string(),
                })
            }

            async fn edit_page(&self, _edit: &EditRequest) -> Result<EditResponse, MwApiError> {
                Ok(EditResponse {
                    result: "Success".to_string(),
                    new_revid: Some(1),
                    new_timestamp: Some(Utc::now().to_rfc3339()),
                })
            }

            async fn parse_wikitext(&self, _wikitext: &str, _title: &Title) -> Result<String, MwApiError> {
                Ok("<html></html>".to_string())
            }
        }

        let config = BotConfig::default();
        let client = SecretLeakingClient;
        let ruleset = RuleSet::new();
        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let mut runner = BotRunner::new(
            config,
            client,
            engine,
            vec!["Page1".to_string(), "Page2".to_string()],
        );
        runner.add_secret("mypassword12345678".to_string());

        // Run the bot - it will fail to fetch pages but record errors
        let report = runner.run().await.unwrap();

        // Verify that both pages errored
        assert_eq!(report.pages_errored, 2);

        // Check that the report contains redacted errors, not raw secrets
        let report_json = serde_json::to_string(&report).unwrap();
        assert!(
            !report_json.contains("mypassword12345678"),
            "Report should not contain raw secret"
        );
        assert!(
            report_json.contains("[REDACTED]"),
            "Report should contain redaction placeholder"
        );
    }

    #[tokio::test]
    async fn test_edit_delay_is_respected() {
        // Mock client that tracks edit timestamps
        use std::sync::Mutex;

        struct TimingClient {
            pages: std::collections::HashMap<String, PageContent>,
            edit_times: Arc<Mutex<Vec<Instant>>>,
        }

        impl TimingClient {
            fn new() -> Self {
                Self {
                    pages: std::collections::HashMap::new(),
                    edit_times: Arc::new(Mutex::new(Vec::new())),
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
        impl MediaWikiClient for TimingClient {
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
                Ok("token".to_string())
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
                // Record the time of this edit
                self.edit_times.lock().unwrap().push(Instant::now());

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

        // Create config with 1 second delay for faster testing
        let config = BotConfig::default()
            .with_edit_delay(Duration::from_secs(1))
            .with_skip_no_change(false);

        let mut client = TimingClient::new();
        let edit_times = client.edit_times.clone();

        // Add pages with content that will trigger edits
        client.add_page("Page1", "test  content"); // double space will be fixed
        client.add_page("Page2", "test  content"); // double space will be fixed

        let mut ruleset = RuleSet::new();
        ruleset.add(awb_domain::rules::Rule::new_plain("  ", " ", true));

        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let mut runner = BotRunner::new(
            config,
            client,
            engine,
            vec!["Page1".to_string(), "Page2".to_string()],
        );

        // Run the bot
        let _report = runner.run().await.unwrap();

        // Verify that we had 2 edits with delay between them
        let times = edit_times.lock().unwrap();
        assert_eq!(times.len(), 2, "Should have made 2 edits");

        // Check that the delay between edits is at least 900ms (allowing for timing variance)
        let delay = times[1].duration_since(times[0]);
        assert!(
            delay >= Duration::from_millis(900),
            "Delay between edits should be at least 900ms, but was {:?}",
            delay
        );
    }

    #[tokio::test]
    async fn test_edit_conflict_retry_and_resolve() {
        use std::sync::Arc;
        use tokio::sync::RwLock;

        // Mock client that returns EditConflict on first edit, Success on second
        struct ConflictThenSuccessClient {
            attempt_count: Arc<RwLock<u32>>,
        }

        impl ConflictThenSuccessClient {
            fn new() -> Self {
                Self {
                    attempt_count: Arc::new(RwLock::new(0)),
                }
            }
        }

        #[async_trait]
        impl MediaWikiClient for ConflictThenSuccessClient {
            async fn login_bot_password(&self, _username: &str, _password: &str) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn login_oauth1(&self, _config: OAuth1Config) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn login_oauth2(&self, _session: OAuthSession) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn fetch_csrf_token(&self) -> Result<String, MwApiError> {
                Ok("token".to_string())
            }

            async fn get_page(&self, title: &Title) -> Result<PageContent, MwApiError> {
                // Return different content on refetch to simulate another edit
                let attempt = *self.attempt_count.read().await;
                let wikitext = if attempt == 0 {
                    "original content"
                } else {
                    "content modified by someone else"
                };

                Ok(PageContent {
                    page_id: PageId(1),
                    title: title.clone(),
                    revision: RevisionId(100 + attempt as u64),
                    timestamp: Utc::now(),
                    wikitext: wikitext.to_string(),
                    size_bytes: wikitext.len() as u64,
                    is_redirect: false,
                    protection: ProtectionInfo::default(),
                    properties: PageProperties::default(),
                })
            }

            async fn edit_page(&self, _edit: &EditRequest) -> Result<EditResponse, MwApiError> {
                let mut count = self.attempt_count.write().await;
                *count += 1;

                if *count == 1 {
                    // First attempt: return conflict
                    Err(MwApiError::EditConflict {
                        base_rev: RevisionId(100),
                        current_rev: RevisionId(101),
                    })
                } else {
                    // Second attempt: succeed
                    Ok(EditResponse {
                        result: "Success".to_string(),
                        new_revid: Some(102),
                        new_timestamp: Some(Utc::now().to_rfc3339()),
                    })
                }
            }

            async fn parse_wikitext(&self, _wikitext: &str, _title: &Title) -> Result<String, MwApiError> {
                Ok("<html></html>".to_string())
            }
        }

        let config = BotConfig::default().with_skip_no_change(false);
        let client = ConflictThenSuccessClient::new();

        let mut ruleset = RuleSet::new();
        // Add a rule that will modify the text
        ruleset.add(awb_domain::rules::Rule::new_plain("content", "FIXED", true));

        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let runner = BotRunner::new(config, client, engine, vec!["TestPage".to_string()]);
        let result = runner.process_page("TestPage").await.unwrap();

        // Should succeed after retry
        assert_eq!(result.action, PageAction::Edited);
        assert!(result.diff_summary.unwrap().contains("rules applied"));
    }

    #[tokio::test]
    async fn test_edit_conflict_retry_twice_then_skip() {
        // Mock client that always returns EditConflict
        struct AlwaysConflictClient;

        #[async_trait]
        impl MediaWikiClient for AlwaysConflictClient {
            async fn login_bot_password(&self, _username: &str, _password: &str) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn login_oauth1(&self, _config: OAuth1Config) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn login_oauth2(&self, _session: OAuthSession) -> Result<(), MwApiError> {
                Ok(())
            }

            async fn fetch_csrf_token(&self) -> Result<String, MwApiError> {
                Ok("token".to_string())
            }

            async fn get_page(&self, title: &Title) -> Result<PageContent, MwApiError> {
                Ok(PageContent {
                    page_id: PageId(1),
                    title: title.clone(),
                    revision: RevisionId(100),
                    timestamp: Utc::now(),
                    wikitext: "some content".to_string(),
                    size_bytes: 12,
                    is_redirect: false,
                    protection: ProtectionInfo::default(),
                    properties: PageProperties::default(),
                })
            }

            async fn edit_page(&self, _edit: &EditRequest) -> Result<EditResponse, MwApiError> {
                // Always return conflict
                Err(MwApiError::EditConflict {
                    base_rev: RevisionId(100),
                    current_rev: RevisionId(101),
                })
            }

            async fn parse_wikitext(&self, _wikitext: &str, _title: &Title) -> Result<String, MwApiError> {
                Ok("<html></html>".to_string())
            }
        }

        let config = BotConfig::default().with_skip_no_change(false);
        let client = AlwaysConflictClient;

        let mut ruleset = RuleSet::new();
        ruleset.add(awb_domain::rules::Rule::new_plain("content", "FIXED", true));

        let registry = FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let runner = BotRunner::new(config, client, engine, vec!["TestPage".to_string()]);
        let result = runner.process_page("TestPage").await.unwrap();

        // Should be skipped after two conflicts
        assert_eq!(result.action, PageAction::Skipped);
        assert!(result.diff_summary.unwrap().contains("Edit conflict persisted after retry"));
    }
}
