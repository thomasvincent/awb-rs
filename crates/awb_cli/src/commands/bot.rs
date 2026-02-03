use anyhow::{Context, Result};
use awb_bot::{BotConfig, BotRunner, Checkpoint};
use awb_domain::profile::AuthMethod;
use awb_domain::rules::RuleSet;
use awb_engine::general_fixes::FixRegistry;
use awb_engine::transform::TransformEngine;
use awb_mw_api::client::{MediaWikiClient, ReqwestMwClient};
use awb_security::{CredentialPort, InMemoryCredentialStore};
use awb_storage::TomlConfigStore;
use console::style;
use std::collections::HashSet;
use std::path::PathBuf;
use url::Url;

/// Arguments for the bot run command
pub struct BotRunArgs {
    pub wiki: Url,
    pub profile_path: PathBuf,
    pub max_edits: Option<u32>,
    pub dry_run: bool,
    pub checkpoint_path: Option<PathBuf>,
    pub auth_profile: String,
    pub skip_no_change: bool,
    pub skip_on_warning: bool,
    pub log_every_n: u32,
}

pub async fn run(args: BotRunArgs) -> Result<()> {
    println!("{}", style("AWB-RS Bot Mode").bold().cyan());
    println!("Wiki: {}", args.wiki);
    println!("Profile: {}", args.profile_path.display());
    println!(
        "Mode: {}",
        if args.dry_run {
            style("DRY-RUN").yellow()
        } else {
            style("AUTONOMOUS").green().bold()
        }
    );
    if let Some(max) = args.max_edits {
        println!("Max edits: {}", max);
    }
    println!();

    // Load profile
    let config_store = TomlConfigStore::new(&args.profile_path);
    let profile = config_store
        .load_profile(&args.auth_profile)
        .context("Failed to load profile. Create one first or use a different auth-profile.")?;

    // Get credentials
    let cred_store = InMemoryCredentialStore::new();
    let password = cred_store
        .get_password(&args.auth_profile)
        .context("No stored credentials found. Run 'login' command first.")?;

    // Create client and login
    let client = ReqwestMwClient::new(args.wiki.clone(), profile.throttle_policy.clone())
        .context("Failed to create HTTP client")?;

    print!("Logging in... ");
    let username = match &profile.auth_method {
        AuthMethod::BotPassword { username } => username.clone(),
        AuthMethod::OAuth2 { .. } => {
            anyhow::bail!("OAuth2 not yet implemented");
        }
        AuthMethod::OAuth1 { .. } => {
            anyhow::bail!("OAuth1 not yet implemented");
        }
    };

    client
        .login_bot_password(&username, &password)
        .await
        .context("Login failed")?;
    println!("{}", style("✓").green().bold());

    // Fetch CSRF token
    print!("Fetching CSRF token... ");
    client
        .fetch_csrf_token()
        .await
        .context("Failed to fetch CSRF token")?;
    println!("{}", style("✓").green().bold());

    // Load rules and build engine
    let ruleset = RuleSet::new(); // In production, load from profile
    let registry = FixRegistry::with_defaults();
    let enabled_fixes = HashSet::new(); // In production, load from profile

    let engine = TransformEngine::new(&ruleset, registry, enabled_fixes)
        .context("Failed to create transform engine")?;

    // For demo purposes, generate a simple page list
    // In real usage, this would come from the profile configuration or a list command
    let pages = vec![
        "Test Page 1".to_string(),
        "Test Page 2".to_string(),
        "Test Page 3".to_string(),
    ];

    println!("Processing {} pages...", pages.len());
    println!();

    // Configure bot
    let mut bot_config = BotConfig::new()
        .with_skip_no_change(args.skip_no_change)
        .with_skip_on_warning(args.skip_on_warning)
        .with_log_every_n(args.log_every_n)
        .with_dry_run(args.dry_run);

    if let Some(max) = args.max_edits {
        bot_config = bot_config.with_max_edits(max);
    }

    // Load or create checkpoint
    let checkpoint = if let Some(ref path) = args.checkpoint_path {
        if path.exists() {
            println!("Loading checkpoint from {}...", path.display());
            Checkpoint::load(path).context("Failed to load checkpoint")?
        } else {
            Checkpoint::new()
        }
    } else {
        Checkpoint::new()
    };

    // Create and run bot
    let mut bot_runner = if checkpoint.next_index() > 0 {
        println!(
            "Resuming from page {} (checkpoint)",
            checkpoint.next_index() + 1
        );
        BotRunner::with_checkpoint(bot_config, client, engine, pages, checkpoint)
    } else {
        BotRunner::new(bot_config, client, engine, pages)
    };

    // Register secrets for redaction in error messages
    bot_runner.add_secret(password.clone());

    let report = match bot_runner.run().await {
        Ok(report) => report,
        Err(e) => {
            eprintln!("{} Bot error: {}", style("✗").red(), e);

            // Save checkpoint on error
            if let Some(path) = args.checkpoint_path {
                if let Err(e) = bot_runner.save_checkpoint(&path) {
                    eprintln!("{} Failed to save checkpoint: {}", style("✗").red(), e);
                } else {
                    println!("{} Checkpoint saved for resume", style("ℹ").cyan());
                }
            }

            return Err(e.into());
        }
    };

    // Save final checkpoint
    if let Some(path) = args.checkpoint_path {
        bot_runner
            .save_checkpoint(&path)
            .context("Failed to save final checkpoint")?;
    }

    // Display report
    println!();
    println!("{}", style("═".repeat(60)).dim());
    println!("{}", report.to_summary());
    println!("{}", style("═".repeat(60)).dim());

    // Save JSON report
    let report_path = PathBuf::from(format!(
        "bot-report-{}.json",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));
    std::fs::write(&report_path, report.to_json()?).context("Failed to save report")?;
    println!("Report saved to: {}", report_path.display());

    Ok(())
}
