use crate::error::MwApiError;
use reqwest::Client;
use serde::Deserialize;
use tracing::info;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct LoginResponse {
    login: LoginResult,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct LoginResult {
    result: String,
    #[serde(default)]
    reason: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TokenResponse {
    query: TokenQuery,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TokenQuery {
    tokens: Tokens,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Tokens {
    #[serde(rename = "logintoken")]
    login_token: Option<String>,
    #[serde(rename = "csrftoken")]
    csrf_token: Option<String>,
}

pub async fn fetch_login_token(client: &Client, api_url: &url::Url) -> Result<String, MwApiError> {
    let resp: serde_json::Value = client
        .get(api_url.as_str())
        .query(&[
            ("action", "query"),
            ("meta", "tokens"),
            ("type", "login"),
            ("format", "json"),
        ])
        .send()
        .await?
        .json()
        .await?;

    resp["query"]["tokens"]["logintoken"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| MwApiError::AuthError {
            reason: "No login token returned".into(),
        })
}

pub async fn login_bot_password(
    client: &Client,
    api_url: &url::Url,
    username: &str,
    password: &str,
) -> Result<(), MwApiError> {
    let token = fetch_login_token(client, api_url).await?;

    let resp: serde_json::Value = client
        .post(api_url.as_str())
        .form(&[
            ("action", "login"),
            ("lgname", username),
            ("lgpassword", password),
            ("lgtoken", &token),
            ("format", "json"),
        ])
        .send()
        .await?
        .json()
        .await?;

    let result = resp["login"]["result"].as_str().unwrap_or("");
    if result == "Success" {
        info!(username, "Login successful");
        Ok(())
    } else {
        let reason = resp["login"]["reason"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string();
        Err(MwApiError::AuthError { reason })
    }
}

pub async fn fetch_csrf_token(client: &Client, api_url: &url::Url) -> Result<String, MwApiError> {
    let resp: serde_json::Value = client
        .get(api_url.as_str())
        .query(&[
            ("action", "query"),
            ("meta", "tokens"),
            ("type", "csrf"),
            ("format", "json"),
        ])
        .send()
        .await?
        .json()
        .await?;

    resp["query"]["tokens"]["csrftoken"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| MwApiError::AuthError {
            reason: "No CSRF token returned".into(),
        })
}
