use anyhow::{Context, Result};
use awb_domain::profile::AuthMethod;
use awb_domain::types::Title;
use awb_engine::diff_engine::{compute_diff, to_unified};
use awb_mw_api::client::{MediaWikiClient, ReqwestMwClient, EditRequest};
use awb_security::{CredentialPort, InMemoryCredentialStore};
use awb_storage::TomlConfigStore;
use chrono::Utc;
use console::style;
use dialoguer::Select;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use url::Url;

pub async fn run(
    wiki: Url,
    profile_path: PathBuf,
    batch: bool,
    dry_run: bool,
    auth_profile: String,
) -> Result<()> {
    println!("{}", style("AWB-RS Edit Workflow").bold().cyan());
    println!("Wiki: {}", wiki);
    println!("Profile: {}", profile_path.display());
    println!("Mode: {}", if dry_run {
        style("DRY-RUN").yellow()
    } else if batch {
        style("BATCH").green()
    } else {
        style("INTERACTIVE").cyan()
    });
    println!();

    // Load profile
    let config_store = TomlConfigStore::new(&profile_path);
    let profile = config_store.load_profile(&auth_profile)
        .context("Failed to load profile. Create one first or use a different auth-profile.")?;

    // Get credentials
    let cred_store = InMemoryCredentialStore::new();
    let password = cred_store.get_password(&auth_profile)
        .context("No stored credentials found. Run 'login' command first.")?;

    // Create client and login
    let client = ReqwestMwClient::new(wiki.clone(), profile.throttle_policy.clone());

    print!("Logging in... ");
    let username = match &profile.auth_method {
        AuthMethod::BotPassword { username } => username.clone(),
        AuthMethod::OAuth2 { .. } => {
            anyhow::bail!("OAuth2 not yet implemented");
        }
    };

    client.login_bot_password(&username, &password)
        .await
        .context("Login failed")?;
    println!("{}", style("✓").green().bold());

    // Fetch CSRF token
    print!("Fetching CSRF token... ");
    client.fetch_csrf_token().await.context("Failed to fetch CSRF token")?;
    println!("{}", style("✓").green().bold());

    // For demo purposes, generate a simple page list
    // In real usage, this would come from the profile configuration
    let titles = vec![
        Title::new(awb_domain::types::Namespace::MAIN, "Test Page 1"),
        Title::new(awb_domain::types::Namespace::MAIN, "Test Page 2"),
    ];

    println!();
    println!("Processing {} pages...", titles.len());
    println!();

    let pb = ProgressBar::new(titles.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-")
    );

    let mut saved_count = 0;
    let mut skipped_count = 0;

    for title in titles {
        pb.set_message(title.display.clone());

        // Fetch page
        let page = match client.get_page(&title).await {
            Ok(p) => p,
            Err(e) => {
                pb.println(format!("  {} Failed to fetch {}: {}",
                    style("✗").red(),
                    title.display,
                    e
                ));
                skipped_count += 1;
                pb.inc(1);
                continue;
            }
        };

        // Apply transformations (simplified - in real usage would use awb_engine rules)
        let new_text = apply_simple_transform(&page.wikitext);

        if new_text == page.wikitext {
            pb.println(format!("  {} No changes needed: {}",
                style("→").dim(),
                title.display
            ));
            skipped_count += 1;
            pb.inc(1);
            continue;
        }

        // Compute diff
        let diff_ops = compute_diff(&page.wikitext, &new_text);
        let unified_diff = to_unified(&diff_ops, 3);

        // Show diff
        pb.println(format!("\n{}", style(format!("Diff for: {}", title.display)).bold()));
        pb.println(style("─".repeat(60)).dim().to_string());
        for line in unified_diff.lines().take(20) {
            if line.starts_with('+') {
                pb.println(style(line).green().to_string());
            } else if line.starts_with('-') {
                pb.println(style(line).red().to_string());
            } else {
                pb.println(line.to_string());
            }
        }
        pb.println(style("─".repeat(60)).dim().to_string());

        // Decide action
        let should_save = if dry_run {
            pb.println(format!("  {} Dry-run mode - not saving\n", style("ℹ").cyan()));
            false
        } else if batch {
            pb.println(format!("  {} Batch mode - auto-saving\n", style("✓").green()));
            true
        } else {
            // Interactive mode
            let choices = vec!["Save", "Skip", "Stop"];
            let selection = Select::new()
                .with_prompt("Action")
                .items(&choices)
                .default(0)
                .interact()
                .context("Failed to read user input")?;

            match selection {
                0 => true,  // Save
                1 => false, // Skip
                2 => {      // Stop
                    pb.println(format!("\n{}", style("Stopped by user").yellow()));
                    break;
                }
                _ => false,
            }
        };

        if should_save {
            let edit_request = EditRequest {
                title: title.clone(),
                text: new_text,
                summary: "AWB-RS automated edit".to_string(),
                minor: true,
                bot: true,
                base_timestamp: page.timestamp.to_rfc3339(),
                start_timestamp: Utc::now().to_rfc3339(),
                section: None,
            };

            match client.edit_page(&edit_request).await {
                Ok(response) => {
                    pb.println(format!("  {} Saved: {} (rev {})",
                        style("✓").green().bold(),
                        title.display,
                        response.new_revid.unwrap_or(0)
                    ));
                    saved_count += 1;
                }
                Err(e) => {
                    pb.println(format!("  {} Failed to save {}: {}",
                        style("✗").red(),
                        title.display,
                        e
                    ));
                    skipped_count += 1;
                }
            }
        } else {
            pb.println(format!("  {} Skipped: {}\n", style("→").yellow(), title.display));
            skipped_count += 1;
        }

        pb.inc(1);
    }

    pb.finish_with_message("Complete");

    println!();
    println!("{}", style("Summary").bold().cyan());
    println!("  Saved: {}", style(saved_count).green().bold());
    println!("  Skipped: {}", style(skipped_count).yellow());
    println!();

    Ok(())
}

fn apply_simple_transform(wikitext: &str) -> String {
    // Simple example transformation: fix double spaces
    wikitext.replace("  ", " ")
}
