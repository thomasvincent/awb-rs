use crate::error::MwApiError;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    RedirectUrl, TokenResponse as OAuth2TokenResponse, TokenUrl,
    basic::BasicClient, RefreshToken, Scope,
};
use oauth2::reqwest::async_http_client;
use serde::{Deserialize, Serialize};

type HmacSha1 = Hmac<Sha1>;

/// OAuth 1.0a configuration (used by MediaWiki)
#[derive(Clone, Serialize, Deserialize)]
pub struct OAuth1Config {
    pub consumer_key: String,
    pub consumer_secret: String,
    pub access_token: String,
    pub access_secret: String,
}

impl std::fmt::Debug for OAuth1Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuth1Config")
            .field("consumer_key", &self.consumer_key)
            .field("consumer_secret", &"***REDACTED***")
            .field("access_token", &self.access_token)
            .field("access_secret", &"***REDACTED***")
            .finish()
    }
}

/// OAuth 2.0 configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct OAuth2Config {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub token_endpoint: String,
    pub auth_endpoint: String,
}

impl std::fmt::Debug for OAuth2Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuth2Config")
            .field("client_id", &self.client_id)
            .field("client_secret", &"***REDACTED***")
            .field("redirect_uri", &self.redirect_uri)
            .field("token_endpoint", &self.token_endpoint)
            .field("auth_endpoint", &self.auth_endpoint)
            .finish()
    }
}

/// Token response for OAuth 2.0
#[derive(Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    #[serde(skip, default = "SystemTime::now")]
    pub issued_at: SystemTime,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"***REDACTED***")
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "***REDACTED***"))
            .field("expires_in", &self.expires_in)
            .field("issued_at", &self.issued_at)
            .finish()
    }
}

impl TokenResponse {
    /// Check if the token is expired or will expire within the next 60 seconds
    pub fn is_expired(&self) -> bool {
        if let Some(expires_in) = self.expires_in {
            let now = SystemTime::now();
            let elapsed = now.duration_since(self.issued_at).unwrap_or_default();
            elapsed.as_secs() + 60 >= expires_in
        } else {
            false
        }
    }
}

/// OAuth 1.0a signature generation (MediaWiki uses HMAC-SHA1)
pub fn oauth1_sign_request(
    config: &OAuth1Config,
    method: &str,
    url: &str,
    params: &[(String, String)],
) -> Result<String, MwApiError> {
    // Generate nonce and timestamp
    let nonce = generate_nonce();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    // Build OAuth parameters
    let mut oauth_params = BTreeMap::new();
    oauth_params.insert("oauth_consumer_key", config.consumer_key.as_str());
    oauth_params.insert("oauth_token", config.access_token.as_str());
    oauth_params.insert("oauth_signature_method", "HMAC-SHA1");
    oauth_params.insert("oauth_timestamp", &timestamp);
    oauth_params.insert("oauth_nonce", &nonce);
    oauth_params.insert("oauth_version", "1.0");

    // Combine OAuth params with request params
    let mut all_params = BTreeMap::new();
    for (k, v) in oauth_params.iter() {
        all_params.insert(k.to_string(), v.to_string());
    }
    for (k, v) in params {
        all_params.insert(k.clone(), v.clone());
    }

    // Build parameter string
    let param_string = all_params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    // Build signature base string
    let base_string = format!(
        "{}&{}&{}",
        percent_encode(method),
        percent_encode(url),
        percent_encode(&param_string)
    );

    // Build signing key
    let signing_key = format!(
        "{}&{}",
        percent_encode(&config.consumer_secret),
        percent_encode(&config.access_secret)
    );

    // Generate signature
    let mut mac = HmacSha1::new_from_slice(signing_key.as_bytes())
        .map_err(|e| MwApiError::AuthError { reason: format!("HMAC error: {}", e) })?;
    mac.update(base_string.as_bytes());
    use base64::Engine;
    let signature = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

    oauth_params.insert("oauth_signature", &signature);

    // Build Authorization header
    let auth_header = oauth_params
        .iter()
        .map(|(k, v)| format!(r#"{}="{}""#, k, percent_encode(v)))
        .collect::<Vec<_>>()
        .join(", ");

    Ok(format!("OAuth {}", auth_header))
}

/// Generate a random nonce for OAuth 1.0a
fn generate_nonce() -> String {
    use rand::Rng;
    let nonce: u128 = rand::rngs::OsRng.r#gen();
    format!("{:x}", nonce)
}

/// Percent-encode a string according to RFC 3986
fn percent_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Generate OAuth 2.0 authorization URL
pub async fn oauth2_authorization_url(config: &OAuth2Config) -> Result<(String, String), MwApiError> {
    let client = build_oauth2_client(config)?;

    let (auth_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("editpage".to_string()))
        .add_scope(Scope::new("createeditmovepage".to_string()))
        .url();

    Ok((auth_url.to_string(), csrf_state.secret().clone()))
}

/// Exchange authorization code for access token
pub async fn oauth2_exchange_code(
    config: &OAuth2Config,
    code: &str,
    expected_state: &str,
    received_state: &str,
) -> Result<TokenResponse, MwApiError> {
    // Validate CSRF state to prevent attacks
    if expected_state != received_state {
        return Err(MwApiError::AuthError {
            reason: "OAuth2 state mismatch - possible CSRF attack".into(),
        });
    }

    let client = build_oauth2_client(config)?;

    let token_result = client
        .exchange_code(AuthorizationCode::new(code.to_string()))
        .request_async(async_http_client)
        .await
        .map_err(|e| MwApiError::AuthError {
            reason: format!("OAuth2 token exchange failed: {}", e)
        })?;

    Ok(TokenResponse {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
        expires_in: token_result.expires_in().map(|d| d.as_secs()),
        issued_at: SystemTime::now(),
    })
}

/// Refresh an expired OAuth 2.0 token
pub async fn oauth2_refresh_token(
    config: &OAuth2Config,
    refresh_token: &str,
) -> Result<TokenResponse, MwApiError> {
    let client = build_oauth2_client(config)?;

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
        .request_async(async_http_client)
        .await
        .map_err(|e| MwApiError::AuthError {
            reason: format!("OAuth2 token refresh failed: {}", e)
        })?;

    Ok(TokenResponse {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
        expires_in: token_result.expires_in().map(|d| d.as_secs()),
        issued_at: SystemTime::now(),
    })
}

/// Build OAuth 2.0 client
fn build_oauth2_client(config: &OAuth2Config) -> Result<BasicClient, MwApiError> {
    let auth_url = AuthUrl::new(config.auth_endpoint.clone())
        .map_err(|e| MwApiError::AuthError {
            reason: format!("Invalid auth URL: {}", e)
        })?;

    let token_url = TokenUrl::new(config.token_endpoint.clone())
        .map_err(|e| MwApiError::AuthError {
            reason: format!("Invalid token URL: {}", e)
        })?;

    Ok(BasicClient::new(
        ClientId::new(config.client_id.clone()),
        Some(ClientSecret::new(config.client_secret.clone())),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(
        RedirectUrl::new(config.redirect_uri.clone())
            .map_err(|e| MwApiError::AuthError {
                reason: format!("Invalid redirect URI: {}", e)
            })?
    ))
}

/// OAuth session manager (handles token lifecycle)
#[derive(Debug, Clone)]
pub struct OAuthSession {
    config: OAuth2Config,
    token: TokenResponse,
}

impl OAuthSession {
    pub fn new(config: OAuth2Config, token: TokenResponse) -> Self {
        Self { config, token }
    }

    /// Get access token, refreshing if needed
    pub async fn get_access_token(&mut self) -> Result<String, MwApiError> {
        if self.token.is_expired() {
            if let Some(ref refresh_token) = self.token.refresh_token {
                self.token = oauth2_refresh_token(&self.config, refresh_token).await?;
            } else {
                return Err(MwApiError::AuthError {
                    reason: "Token expired and no refresh token available".into(),
                });
            }
        }
        Ok(self.token.access_token.clone())
    }

    /// Check if session has a valid token
    pub fn is_valid(&self) -> bool {
        !self.token.is_expired() || self.token.refresh_token.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth1_signature_generation() {
        let config = OAuth1Config {
            consumer_key: "test_consumer".to_string(),
            consumer_secret: "test_secret".to_string(),
            access_token: "test_token".to_string(),
            access_secret: "token_secret".to_string(),
        };

        let params = vec![
            ("action".to_string(), "query".to_string()),
            ("format".to_string(), "json".to_string()),
        ];

        let result = oauth1_sign_request(
            &config,
            "GET",
            "https://en.wikipedia.org/w/api.php",
            &params,
        );

        assert!(result.is_ok());
        let auth_header = result.unwrap();
        assert!(auth_header.starts_with("OAuth "));
        assert!(auth_header.contains("oauth_consumer_key"));
        assert!(auth_header.contains("oauth_signature"));
    }

    #[test]
    fn test_percent_encode() {
        assert_eq!(percent_encode("hello world"), "hello+world");
        assert_eq!(percent_encode("hello&world"), "hello%26world");
        assert_eq!(percent_encode("100%"), "100%25");
    }

    #[test]
    fn test_token_expiry() {
        let mut token = TokenResponse {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_in: Some(3600),
            issued_at: SystemTime::now(),
        };

        // Fresh token should not be expired
        assert!(!token.is_expired());

        // Token issued long ago should be expired
        token.issued_at = SystemTime::now() - std::time::Duration::from_secs(4000);
        assert!(token.is_expired());

        // Token without expiry should never expire
        token.expires_in = None;
        assert!(!token.is_expired());
    }

    #[test]
    fn test_nonce_generation() {
        let nonce1 = generate_nonce();
        let nonce2 = generate_nonce();

        // Nonces should be different
        assert_ne!(nonce1, nonce2);

        // Nonces should be hex strings
        assert!(nonce1.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(nonce2.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
