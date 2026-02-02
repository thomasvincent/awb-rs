use crate::error::MwApiError;
use crate::throttle::ThrottleController;
use crate::retry::RetryPolicy;
use crate::oauth::{OAuth1Config, OAuthSession};
use awb_domain::types::*;
use awb_domain::profile::ThrottlePolicy;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct EditRequest {
    pub title: Title,
    pub text: String,
    pub summary: String,
    pub minor: bool,
    pub bot: bool,
    pub base_timestamp: String,
    pub start_timestamp: String,
    pub section: Option<u32>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EditResponse {
    pub result: String,
    #[serde(rename = "newrevid")]
    pub new_revid: Option<u64>,
    #[serde(rename = "newtimestamp")]
    pub new_timestamp: Option<String>,
}

/// Authentication state for the client
#[derive(Debug, Clone)]
enum AuthState {
    None,
    BotPassword,
    OAuth1 { config: OAuth1Config },
    OAuth2 { session: OAuthSession },
}

#[async_trait]
pub trait MediaWikiClient: Send + Sync {
    async fn login_bot_password(&self, username: &str, password: &str) -> Result<(), MwApiError>;
    async fn login_oauth1(&self, config: OAuth1Config) -> Result<(), MwApiError>;
    async fn login_oauth2(&self, session: OAuthSession) -> Result<(), MwApiError>;
    async fn fetch_csrf_token(&self) -> Result<String, MwApiError>;
    async fn get_page(&self, title: &Title) -> Result<PageContent, MwApiError>;
    async fn edit_page(&self, edit: &EditRequest) -> Result<EditResponse, MwApiError>;
    async fn parse_wikitext(&self, wikitext: &str, title: &Title) -> Result<String, MwApiError>;
}

pub struct ReqwestMwClient {
    http: reqwest::Client,
    api_url: url::Url,
    csrf_token: Arc<RwLock<Option<String>>>,
    auth_state: Arc<RwLock<AuthState>>,
    throttle: ThrottleController,
    #[allow(dead_code)]
    retry_policy: RetryPolicy,
}

impl ReqwestMwClient {
    pub fn new(api_url: url::Url, policy: ThrottlePolicy) -> Self {
        let jar = Arc::new(reqwest::cookie::Jar::default());
        let http = reqwest::ClientBuilder::new()
            .cookie_provider(jar)
            .user_agent("AWB-RS/0.1.0 (https://github.com/thomasvincent/awb-rs)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            api_url,
            csrf_token: Arc::new(RwLock::new(None)),
            auth_state: Arc::new(RwLock::new(AuthState::None)),
            throttle: ThrottleController::new(policy.clone()),
            retry_policy: RetryPolicy {
                max_retries: policy.max_retries,
                ..Default::default()
            },
        }
    }

    /// Apply authentication to a request builder
    async fn apply_auth(&self, mut builder: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder, MwApiError> {
        let auth_state = self.auth_state.read().await;

        match &*auth_state {
            AuthState::None | AuthState::BotPassword => {
                // BotPassword uses cookies, no additional headers needed
                Ok(builder)
            }
            AuthState::OAuth1 { config: _ } => {
                // For OAuth 1.0a, we need to sign each request
                // Extract method and URL from the request (this is a simplified approach)
                // In practice, we'd need to sign with the actual parameters
                drop(auth_state);
                Ok(builder)
            }
            AuthState::OAuth2 { session } => {
                // For OAuth 2.0, add Bearer token
                let session_clone = session.clone();
                drop(auth_state);
                let mut session_clone = session_clone;
                let access_token = session_clone.get_access_token().await?;

                // Update session if token was refreshed
                *self.auth_state.write().await = AuthState::OAuth2 { session: session_clone };

                builder = builder.header("Authorization", format!("Bearer {}", access_token));
                Ok(builder)
            }
        }
    }
}

#[async_trait]
impl MediaWikiClient for ReqwestMwClient {
    async fn login_bot_password(&self, username: &str, password: &str) -> Result<(), MwApiError> {
        crate::auth::login_bot_password(&self.http, &self.api_url, username, password).await?;
        *self.auth_state.write().await = AuthState::BotPassword;
        Ok(())
    }

    async fn login_oauth1(&self, config: OAuth1Config) -> Result<(), MwApiError> {
        *self.auth_state.write().await = AuthState::OAuth1 { config };
        Ok(())
    }

    async fn login_oauth2(&self, session: OAuthSession) -> Result<(), MwApiError> {
        if !session.is_valid() {
            return Err(MwApiError::AuthError {
                reason: "OAuth2 session is invalid or expired".into(),
            });
        }
        *self.auth_state.write().await = AuthState::OAuth2 { session };
        Ok(())
    }

    async fn fetch_csrf_token(&self) -> Result<String, MwApiError> {
        let token = crate::auth::fetch_csrf_token(&self.http, &self.api_url).await?;
        *self.csrf_token.write().await = Some(token.clone());
        Ok(token)
    }

    async fn get_page(&self, title: &Title) -> Result<PageContent, MwApiError> {
        let maxlag = self.throttle.maxlag();
        let builder = self.http.get(self.api_url.as_str())
            .query(&[
                ("action", "query"),
                ("titles", &title.display),
                ("prop", "revisions|info|pageprops"),
                ("rvprop", "ids|timestamp|content"),
                ("rvslots", "main"),
                ("inprop", "protection"),
                ("format", "json"),
                ("maxlag", &maxlag.to_string()),
            ]);

        let builder = self.apply_auth(builder).await?;
        let resp: serde_json::Value = builder.send().await?.json().await?;

        // Check for API errors
        if let Some(error) = resp.get("error") {
            let code = error["code"].as_str().unwrap_or("unknown").to_string();
            if code == "maxlag" {
                let retry_after = error["info"].as_str()
                    .and_then(|s| s.split_whitespace().find_map(|w| w.parse::<u64>().ok()))
                    .unwrap_or(5);
                return Err(MwApiError::MaxLag { retry_after });
            }
            let info = error["info"].as_str().unwrap_or("").to_string();
            return Err(MwApiError::ApiError { code, info });
        }

        // Parse response
        let pages = &resp["query"]["pages"];
        let page = pages.as_object()
            .and_then(|m| m.values().next())
            .ok_or_else(|| MwApiError::ApiError { code: "nopage".into(), info: "No page data returned".into() })?;

        let page_id = PageId(page["pageid"].as_u64().unwrap_or(0));
        let ns = Namespace(page["ns"].as_i64().unwrap_or(0) as i32);
        let page_title = page["title"].as_str().unwrap_or("").to_string();

        let rev = &page["revisions"][0];
        let revision = RevisionId(rev["revid"].as_u64().unwrap_or(0));
        let timestamp_str = rev["timestamp"].as_str().unwrap_or("");
        let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        let wikitext = rev["slots"]["main"]["content"].as_str().unwrap_or("").to_string();

        let is_redirect = page.get("redirect").is_some();

        let protection = {
            let mut info = ProtectionInfo::default();
            if let Some(protections) = page.get("protection").and_then(|p| p.as_array()) {
                for p in protections {
                    let ptype = p["type"].as_str().unwrap_or("");
                    let level = match p["level"].as_str().unwrap_or("") {
                        "autoconfirmed" => Some(ProtectionLevel::Autoconfirmed),
                        "extendedconfirmed" => Some(ProtectionLevel::ExtendedConfirmed),
                        "sysop" => Some(ProtectionLevel::Sysop),
                        _ => None,
                    };
                    match ptype {
                        "edit" => info.edit = level,
                        "move" => info.move_page = level,
                        _ => {}
                    }
                }
            }
            info
        };

        let is_disambig = page.get("pageprops")
            .and_then(|pp| pp.get("disambiguation"))
            .is_some();

        let wikibase_item = page.get("pageprops")
            .and_then(|pp| pp["wikibase_item"].as_str())
            .map(String::from);

        Ok(PageContent {
            page_id,
            title: Title { namespace: ns, name: page_title.clone(), display: page_title },
            revision,
            timestamp,
            wikitext: wikitext.clone(),
            size_bytes: wikitext.len() as u64,
            is_redirect,
            protection,
            properties: PageProperties { is_disambig, wikibase_item },
        })
    }

    async fn edit_page(&self, edit: &EditRequest) -> Result<EditResponse, MwApiError> {
        self.throttle.acquire_edit_permit().await;

        let csrf = {
            let token = self.csrf_token.read().await;
            match token.as_ref() {
                Some(t) => t.clone(),
                None => {
                    drop(token);
                    self.fetch_csrf_token().await?
                }
            }
        };

        let mut params = vec![
            ("action".to_string(), "edit".to_string()),
            ("title".to_string(), edit.title.display.clone()),
            ("text".to_string(), edit.text.clone()),
            ("summary".to_string(), edit.summary.clone()),
            ("token".to_string(), csrf),
            ("basetimestamp".to_string(), edit.base_timestamp.clone()),
            ("starttimestamp".to_string(), edit.start_timestamp.clone()),
            ("format".to_string(), "json".to_string()),
            ("maxlag".to_string(), self.throttle.maxlag().to_string()),
        ];
        if edit.minor { params.push(("minor".to_string(), "1".to_string())); }
        if edit.bot { params.push(("bot".to_string(), "1".to_string())); }
        if let Some(section) = edit.section {
            params.push(("section".to_string(), section.to_string()));
        }

        let builder = self.http.post(self.api_url.as_str())
            .form(&params);
        let builder = self.apply_auth(builder).await?;
        let resp: serde_json::Value = builder.send().await?.json().await?;

        // Check errors
        if let Some(error) = resp.get("error") {
            let code = error["code"].as_str().unwrap_or("unknown").to_string();
            let info = error["info"].as_str().unwrap_or("").to_string();
            return match code.as_str() {
                "editconflict" => Err(MwApiError::EditConflict {
                    base_rev: awb_domain::types::RevisionId(0),
                    current_rev: awb_domain::types::RevisionId(0),
                }),
                "badtoken" => Err(MwApiError::BadToken),
                "maxlag" => Err(MwApiError::MaxLag { retry_after: 5 }),
                _ => Err(MwApiError::ApiError { code, info }),
            };
        }

        let edit_resp = &resp["edit"];
        Ok(EditResponse {
            result: edit_resp["result"].as_str().unwrap_or("").to_string(),
            new_revid: edit_resp["newrevid"].as_u64(),
            new_timestamp: edit_resp["newtimestamp"].as_str().map(String::from),
        })
    }

    async fn parse_wikitext(&self, wikitext: &str, title: &Title) -> Result<String, MwApiError> {
        let builder = self.http.post(self.api_url.as_str())
            .form(&[
                ("action", "parse"),
                ("text", wikitext),
                ("title", &title.display),
                ("contentmodel", "wikitext"),
                ("prop", "text"),
                ("format", "json"),
            ]);
        let builder = self.apply_auth(builder).await?;
        let resp: serde_json::Value = builder.send().await?.json().await?;

        resp["parse"]["text"]["*"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| MwApiError::ApiError { code: "noparse".into(), info: "No parsed HTML returned".into() })
    }
}
