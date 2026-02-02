use anyhow::{Context, Result};
use awb_mw_api::client::{MediaWikiClient, ReqwestMwClient};
use awb_security::{CredentialPort, FileCredentialStore};
use console::style;
use dialoguer::Password;
use url::Url;

pub async fn run(wiki: Url, username: String, profile: String) -> Result<()> {
    println!("{}", style("Login to MediaWiki").bold().cyan());
    println!("Wiki: {}", wiki);
    println!("Username: {}", username);
    println!("Profile: {}", profile);
    println!();

    // Prompt for password
    let password = Password::new()
        .with_prompt("Bot password")
        .interact()
        .context("Failed to read password")?;

    // Create client and attempt login
    let client = ReqwestMwClient::new(wiki.clone(), awb_domain::profile::ThrottlePolicy::default())
        .context("Failed to create HTTP client")?;

    print!("Authenticating... ");
    client.login_bot_password(&username, &password)
        .await
        .context("Authentication failed")?;

    println!("{}", style("✓").green().bold());

    // Store credentials
    let cred_store = FileCredentialStore::new()
        .context("Failed to initialize credential store")?;
    cred_store.set_password(&profile, &password)
        .context("Failed to store credentials")?;

    println!();
    println!("{}", style("Login successful!").green().bold());
    println!("Credentials stored under profile: {}", style(&profile).yellow());
    println!();
    println!("Credentials saved to: ~/.awb-rs/credentials.json");

    Ok(())
}
