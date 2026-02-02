use anyhow::{Context, Result};
use dialoguer::{Input, Password};
use url::Url;
use awb_security::{CredentialPort, KeyringCredentialStore};

pub async fn setup(
    wiki: Url,
    consumer_key: String,
    access_token: String,
    profile: String,
) -> Result<()> {
    use awb_domain::profile::{AuthMethod, Profile, ThrottlePolicy};

    // Validate profile name to prevent path traversal
    if !profile.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        anyhow::bail!("Profile name must contain only alphanumeric characters, hyphens, and underscores");
    }

    println!("Setting up OAuth 1.0a for {}", wiki);

    // Prompt for secrets interactively (never via CLI args)
    let consumer_secret = Password::new()
        .with_prompt("OAuth consumer secret")
        .interact()
        .context("Failed to read consumer secret")?;

    let access_secret = Password::new()
        .with_prompt("OAuth access secret")
        .interact()
        .context("Failed to read access secret")?;

    // Create profile with OAuth1 auth
    let auth_method = AuthMethod::OAuth1 {
        consumer_key: consumer_key.clone(),
        consumer_secret: consumer_secret.clone(),
        access_token: access_token.clone(),
        access_secret: access_secret.clone(),
    };

    let profile_obj = Profile {
        id: profile.clone(),
        name: format!("OAuth Profile for {}", wiki.host_str().unwrap_or("unknown")),
        api_url: wiki.clone(),
        auth_method,
        default_namespaces: std::collections::HashSet::new(),
        throttle_policy: ThrottlePolicy::default(),
    };

    // Store OAuth credentials in OS keychain
    let store = KeyringCredentialStore::new();
    let token_json = serde_json::json!({
        "consumer_key": consumer_key,
        "consumer_secret": consumer_secret,
        "access_token": access_token,
        "access_secret": access_secret,
    })
    .to_string();

    store.set_oauth_token(&profile, &token_json)
        .context("Failed to store OAuth credentials in keychain")?;

    // Save profile
    let profile_path = format!(".awb/profiles/{}.toml", profile);
    std::fs::create_dir_all(".awb/profiles").context("Failed to create profiles directory")?;
    let profile_toml = toml::to_string_pretty(&profile_obj).context("Failed to serialize profile")?;
    std::fs::write(&profile_path, profile_toml)
        .context(format!("Failed to write profile to {}", profile_path))?;

    println!("✓ OAuth 1.0a credentials saved to profile '{}'", profile);
    println!("✓ Profile saved to {}", profile_path);

    Ok(())
}

pub async fn authorize(
    wiki: Url,
    client_id: String,
    profile: String,
) -> Result<()> {
    use awb_mw_api::oauth::{OAuth2Config, oauth2_authorization_url, oauth2_exchange_code};

    // Validate profile name to prevent path traversal
    if !profile.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        anyhow::bail!("Profile name must contain only alphanumeric characters, hyphens, and underscores");
    }

    println!("Starting OAuth 2.0 authorization flow for {}", wiki);

    // Prompt for client secret interactively (never via CLI args)
    let client_secret = Password::new()
        .with_prompt("OAuth 2.0 client secret")
        .interact()
        .context("Failed to read client secret")?;

    // Build OAuth2 config
    // Note: These endpoints are MediaWiki-specific and may need to be customized
    let auth_endpoint = format!("{}?title=Special:OAuth/authorize", wiki.origin().ascii_serialization());
    let token_endpoint = format!("{}?title=Special:OAuth/token", wiki.origin().ascii_serialization());
    let redirect_uri = "http://localhost:8080/callback".to_string();

    let config = OAuth2Config {
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
        redirect_uri: redirect_uri.clone(),
        token_endpoint,
        auth_endpoint,
    };

    // Generate authorization URL with PKCE
    let (auth_url, state, pkce_verifier) = oauth2_authorization_url(&config)
        .await
        .context("Failed to generate authorization URL")?;

    println!("\n🔐 Please visit this URL to authorize:");
    println!("{}\n", auth_url);
    println!("After authorizing, you will be redirected to a callback URL.");
    println!("Copy the 'code' parameter from the URL and paste it here.\n");

    let code: String = Input::new()
        .with_prompt("Authorization code")
        .interact_text()
        .context("Failed to read authorization code")?;

    let received_state: String = Input::new()
        .with_prompt("State parameter from redirect URL")
        .interact_text()
        .context("Failed to read state")?;

    // Exchange code for token with PKCE verifier
    println!("Exchanging authorization code for access token...");
    let token = oauth2_exchange_code(&config, &code, &state, &received_state, &pkce_verifier)
        .await
        .context("Failed to exchange authorization code")?;

    // Store tokens in OS keychain
    let store = KeyringCredentialStore::new();
    let token_json = serde_json::to_string(&token).context("Failed to serialize token")?;
    store.set_oauth_token(&profile, &token_json)
        .context("Failed to store OAuth token in keychain")?;

    // Create and save profile
    use awb_domain::profile::{AuthMethod, Profile, ThrottlePolicy};
    let auth_method = AuthMethod::OAuth2 {
        client_id,
        client_secret,
    };

    let profile_obj = Profile {
        id: profile.clone(),
        name: format!("OAuth2 Profile for {}", wiki.host_str().unwrap_or("unknown")),
        api_url: wiki.clone(),
        auth_method,
        default_namespaces: std::collections::HashSet::new(),
        throttle_policy: ThrottlePolicy::default(),
    };

    let profile_path = format!(".awb/profiles/{}.toml", profile);
    std::fs::create_dir_all(".awb/profiles").context("Failed to create profiles directory")?;
    let profile_toml = toml::to_string_pretty(&profile_obj).context("Failed to serialize profile")?;
    std::fs::write(&profile_path, profile_toml)
        .context(format!("Failed to write profile to {}", profile_path))?;

    println!("✓ OAuth 2.0 token obtained and saved to profile '{}'", profile);
    println!("✓ Profile saved to {}", profile_path);

    if let Some(expires) = token.expires_in {
        println!("ℹ Token will expire in {} seconds", expires);
    }

    Ok(())
}
