// Allow clippy warning in UniFFI-generated code
#![allow(clippy::empty_line_after_doc_comments)]

pub mod c_api;

use awb_domain::profile::ThrottlePolicy;
use awb_domain::rules::RuleSet;
use awb_domain::types::*;
use awb_engine::diff_engine;
use awb_engine::general_fixes::FixRegistry;
use awb_engine::transform::TransformEngine;
use awb_mw_api::client::{EditRequest, MediaWikiClient, ReqwestMwClient};
use parking_lot::Mutex;
use secrecy::SecretString;
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;

// FFI-safe types
#[derive(Clone)]
pub struct SessionHandle {
    pub id: u64,
}

pub struct PageInfo {
    pub page_id: u64,
    pub title: String,
    pub revision: u64,
    pub timestamp: String,
    pub wikitext: String,
    pub size_bytes: u64,
    pub is_redirect: bool,
}

pub struct TransformResult {
    pub new_wikitext: String,
    pub rules_applied: Vec<String>,
    pub fixes_applied: Vec<String>,
    pub summary: String,
    pub warnings: Vec<String>,
    pub diff_html: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FfiError {
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Authentication failed")]
    AuthenticationError,
    #[error("Resource not found")]
    NotFound,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Session not found")]
    SessionNotFound,
    #[error("Lock poisoned")]
    LockPoisoned,
    #[error("Engine error: {0}")]
    EngineError(String),
}

// Session storage with API client
struct Session {
    wiki_url: Url,
    username: String,
    password: Option<SecretString>,
    client: Option<Arc<ReqwestMwClient>>,
    authenticated: bool,
}

lazy_static::lazy_static! {
    static ref SESSIONS: Arc<Mutex<HashMap<u64, Session>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref NEXT_SESSION_ID: Arc<Mutex<u64>> = Arc::new(Mutex::new(1));
    static ref TOKIO_RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
}

// UniFFI exported functions
pub fn create_session(
    wiki_url: String,
    username: String,
    password: String,
) -> Result<SessionHandle, FfiError> {
    // Validate that wiki_url is not empty
    if wiki_url.trim().is_empty() {
        return Err(FfiError::ParseError("wiki_url cannot be empty".to_string()));
    }

    // Parse the URL
    let parsed_url = Url::parse(&wiki_url)
        .map_err(|e| FfiError::ParseError(format!("Invalid wiki URL: {}", e)))?;

    let mut sessions = SESSIONS.lock();
    let mut next_id = NEXT_SESSION_ID.lock();

    let id = *next_id;
    *next_id = next_id
        .checked_add(1)
        .ok_or(FfiError::EngineError("session ID overflow".into()))?;

    sessions.insert(
        id,
        Session {
            wiki_url: parsed_url,
            username,
            password: Some(SecretString::new(password.into())),
            client: None,
            authenticated: false,
        },
    );

    Ok(SessionHandle { id })
}

pub fn destroy_session(handle: SessionHandle) -> Result<(), FfiError> {
    let mut sessions = SESSIONS.lock();
    sessions
        .remove(&handle.id)
        .ok_or(FfiError::SessionNotFound)?;
    Ok(())
}

pub fn login(handle: SessionHandle) -> Result<(), FfiError> {
    let mut sessions = SESSIONS.lock();
    let session = sessions
        .get_mut(&handle.id)
        .ok_or(FfiError::SessionNotFound)?;

    // Create the API client if not already created
    let client = ReqwestMwClient::new(session.wiki_url.clone(), ThrottlePolicy::default())
        .map_err(|e| FfiError::NetworkError(format!("Failed to create API client: {}", e)))?;

    let client = Arc::new(client);

    // Get password before async block
    let password = session
        .password
        .take()
        .ok_or(FfiError::AuthenticationError)?;

    let username = session.username.clone();

    // Store client reference for async block
    let client_clone = client.clone();

    // Run the async login
    TOKIO_RUNTIME
        .block_on(async {
            use secrecy::ExposeSecret;
            client_clone
                .login_bot_password(&username, password.expose_secret())
                .await
        })
        .map_err(|e| FfiError::NetworkError(format!("Login failed: {}", e)))?;

    // Fetch CSRF token
    TOKIO_RUNTIME
        .block_on(async { client.fetch_csrf_token().await })
        .map_err(|e| FfiError::NetworkError(format!("Failed to fetch CSRF token: {}", e)))?;

    session.client = Some(client);
    session.authenticated = true;

    Ok(())
}

pub fn fetch_list(
    handle: SessionHandle,
    source: String,
    query: String,
) -> Result<Vec<String>, FfiError> {
    let sessions = SESSIONS.lock();
    let session = sessions.get(&handle.id).ok_or(FfiError::SessionNotFound)?;

    if !session.authenticated {
        return Err(FfiError::AuthenticationError);
    }

    // For now, return a mock list since MediaWiki API list fetching
    // requires category/search/whatlinks queries which aren't implemented in awb_mw_api yet
    // TODO: Implement actual list fetching when awb_mw_api supports it
    Ok(vec![
        format!("Page from {}: {}", source, query),
        "Example Page 1".to_string(),
        "Example Page 2".to_string(),
    ])
}

pub fn get_page(handle: SessionHandle, title: String) -> Result<PageInfo, FfiError> {
    let sessions = SESSIONS.lock();
    let session = sessions.get(&handle.id).ok_or(FfiError::SessionNotFound)?;

    let client = session
        .client
        .as_ref()
        .ok_or(FfiError::AuthenticationError)?
        .clone();

    drop(sessions); // Release lock before async operation

    let page_title = Title::new(Namespace::MAIN, &title);

    let page = TOKIO_RUNTIME
        .block_on(async { client.get_page(&page_title).await })
        .map_err(|e| match e {
            awb_mw_api::error::MwApiError::ApiError { code, .. } if code == "missingtitle" => {
                FfiError::NotFound
            }
            _ => FfiError::NetworkError(format!("Failed to fetch page: {}", e)),
        })?;

    Ok(PageInfo {
        page_id: page.page_id.0,
        title: page.title.display.clone(),
        revision: page.revision.0,
        timestamp: page.timestamp.to_rfc3339(),
        wikitext: page.wikitext,
        size_bytes: page.size_bytes,
        is_redirect: page.is_redirect,
    })
}

pub fn apply_rules(
    handle: SessionHandle,
    content: String,
    rules_json: String,
) -> Result<TransformResult, FfiError> {
    let sessions = SESSIONS.lock();
    let _session = sessions.get(&handle.id).ok_or(FfiError::SessionNotFound)?;

    // Parse rules from JSON
    let rule_set: RuleSet = serde_json::from_str(&rules_json)
        .map_err(|e| FfiError::ParseError(format!("Invalid rules JSON: {}", e)))?;

    // Create a mock page content
    let page = PageContent {
        page_id: PageId(1),
        title: Title::new(Namespace::MAIN, "Test"),
        revision: RevisionId(1),
        timestamp: chrono::Utc::now(),
        wikitext: content.clone(),
        size_bytes: content.len() as u64,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    };

    // Apply transformations
    let fix_registry = FixRegistry::with_defaults();
    let enabled_fixes = std::collections::HashSet::new();
    let engine = TransformEngine::new(&rule_set, fix_registry, enabled_fixes)
        .map_err(|e| FfiError::EngineError(format!("Transform engine error: {}", e)))?;

    let plan = engine.apply(&page);

    // Convert diff to HTML
    let diff_html = format_diff_as_html(&plan.diff_ops);

    Ok(TransformResult {
        new_wikitext: plan.new_wikitext,
        rules_applied: plan.rules_applied.iter().map(|id| id.to_string()).collect(),
        fixes_applied: plan.fixes_applied,
        summary: plan.summary,
        warnings: plan.warnings.iter().map(|w| format!("{:?}", w)).collect(),
        diff_html,
    })
}

pub fn save_page(
    handle: SessionHandle,
    title: String,
    content: String,
    summary: String,
) -> Result<(), FfiError> {
    // Validate inputs
    if title.is_empty() {
        return Err(FfiError::ParseError("Title cannot be empty".to_string()));
    }
    if content.is_empty() {
        return Err(FfiError::ParseError("Content cannot be empty".to_string()));
    }
    if summary.is_empty() {
        return Err(FfiError::ParseError("Summary cannot be empty".to_string()));
    }

    let sessions = SESSIONS.lock();
    let session = sessions.get(&handle.id).ok_or(FfiError::SessionNotFound)?;

    let client = session
        .client
        .as_ref()
        .ok_or(FfiError::AuthenticationError)?
        .clone();

    drop(sessions); // Release lock before async operation

    let page_title = Title::new(Namespace::MAIN, &title);

    // First fetch the page to get base timestamp
    let page = TOKIO_RUNTIME
        .block_on(async { client.get_page(&page_title).await })
        .map_err(|e| FfiError::NetworkError(format!("Failed to fetch page for edit: {}", e)))?;

    let edit_request = EditRequest {
        title: page_title,
        text: content,
        summary,
        minor: true,
        bot: true,
        base_timestamp: page.timestamp.to_rfc3339(),
        start_timestamp: chrono::Utc::now().to_rfc3339(),
        section: None,
    };

    let response = TOKIO_RUNTIME
        .block_on(async { client.edit_page(&edit_request).await })
        .map_err(|e| FfiError::NetworkError(format!("Failed to save page: {}", e)))?;

    if response.result != "Success" {
        return Err(FfiError::NetworkError(format!(
            "Edit failed: {}",
            response.result
        )));
    }

    Ok(())
}

pub fn compute_diff(old_text: String, new_text: String) -> String {
    let diff_ops = diff_engine::compute_diff(&old_text, &new_text);
    format_diff_as_html(&diff_ops)
}

fn format_diff_as_html(diff_ops: &[awb_domain::diff::DiffOp]) -> String {
    use awb_domain::diff::DiffOp;

    let mut html = String::from("<div class='diff'>");

    for op in diff_ops {
        match op {
            DiffOp::Equal { text, .. } => {
                html.push_str(&format!("<span class='equal'>{}</span>", html_escape(text)));
            }
            DiffOp::Delete { text, .. } => {
                html.push_str(&format!(
                    "<span class='delete'>{}</span>",
                    html_escape(text)
                ));
            }
            DiffOp::Insert { text, .. } => {
                html.push_str(&format!(
                    "<span class='insert'>{}</span>",
                    html_escape(text)
                ));
            }
            DiffOp::Replace {
                old_text, new_text, ..
            } => {
                html.push_str(&format!(
                    "<span class='delete'>{}</span>",
                    html_escape(old_text)
                ));
                html.push_str(&format!(
                    "<span class='insert'>{}</span>",
                    html_escape(new_text)
                ));
            }
        }
    }

    html.push_str("</div>");
    html
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// Include the UniFFI scaffolding
uniffi::include_scaffolding!("awb_ffi");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let handle = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        )
        .unwrap();
        assert!(handle.id > 0);
    }

    #[test]
    fn test_create_session_returns_valid_handle() {
        let handle = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        )
        .unwrap();

        assert!(handle.id > 0, "Handle ID should be positive");
    }

    #[test]
    fn test_create_session_increments_id() {
        let handle1 = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "user1".to_string(),
            "pass1".to_string(),
        )
        .unwrap();

        let handle2 = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "user2".to_string(),
            "pass2".to_string(),
        )
        .unwrap();

        assert!(handle2.id > handle1.id, "Session IDs should increment");
    }

    #[test]
    fn test_create_session_with_empty_url() {
        let result = create_session("".to_string(), "user".to_string(), "pass".to_string());

        // Empty URL should now be rejected
        assert!(result.is_err());
        match result {
            Err(FfiError::ParseError(msg)) => assert!(msg.contains("wiki_url")),
            _ => panic!("Expected ParseError for empty wiki_url"),
        }
    }

    #[test]
    fn test_create_session_with_invalid_url() {
        let result = create_session(
            "not a valid url".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        assert!(result.is_err());
        match result {
            Err(FfiError::ParseError(msg)) => assert!(msg.contains("Invalid wiki URL")),
            _ => panic!("Expected ParseError for invalid URL"),
        }
    }

    #[test]
    fn test_create_session_with_empty_username() {
        let result = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "".to_string(),
            "pass".to_string(),
        );

        // Empty username should still create session
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_session_with_empty_password() {
        let result = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "user".to_string(),
            "".to_string(),
        );

        // Empty password should still create session
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_session_with_all_empty_strings() {
        let result = create_session("".to_string(), "".to_string(), "".to_string());

        // Empty wiki_url should be rejected
        assert!(result.is_err(), "Should reject empty wiki_url");
        match result {
            Err(FfiError::ParseError(msg)) => assert!(msg.contains("wiki_url")),
            _ => panic!("Expected ParseError for empty wiki_url"),
        }
    }

    #[test]
    fn test_save_page_validates_title() {
        let handle = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "user".to_string(),
            "pass".to_string(),
        )
        .unwrap();

        let result = save_page(
            handle,
            "".to_string(), // Empty title
            "content".to_string(),
            "summary".to_string(),
        );

        assert!(result.is_err());
        match result {
            Err(FfiError::ParseError(msg)) => assert!(msg.contains("Title")),
            _ => panic!("Expected ParseError for empty title"),
        }
    }

    #[test]
    fn test_save_page_validates_content() {
        let handle = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "user".to_string(),
            "pass".to_string(),
        )
        .unwrap();

        let result = save_page(
            handle,
            "Test Page".to_string(),
            "".to_string(), // Empty content
            "summary".to_string(),
        );

        assert!(result.is_err());
        match result {
            Err(FfiError::ParseError(msg)) => assert!(msg.contains("Content")),
            _ => panic!("Expected ParseError for empty content"),
        }
    }

    #[test]
    fn test_save_page_validates_summary() {
        let handle = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "user".to_string(),
            "pass".to_string(),
        )
        .unwrap();

        let result = save_page(
            handle,
            "Test Page".to_string(),
            "content".to_string(),
            "".to_string(), // Empty summary
        );

        assert!(result.is_err());
        match result {
            Err(FfiError::ParseError(msg)) => assert!(msg.contains("Summary")),
            _ => panic!("Expected ParseError for empty summary"),
        }
    }

    #[test]
    fn test_compute_diff_basic() {
        let old = "line1\nline2\nline3".to_string();
        let new = "line1\nmodified\nline3".to_string();

        let diff = compute_diff(old, new);

        assert!(diff.contains("line1"));
        assert!(diff.contains("line3"));
        assert!(diff.contains("<div class='diff'>"));
        assert!(diff.contains("</div>"));
    }

    #[test]
    fn test_compute_diff_empty_strings() {
        let old = "".to_string();
        let new = "".to_string();

        let diff = compute_diff(old, new);

        assert!(diff.contains("<div class='diff'>"));
        assert!(diff.contains("</div>"));
    }

    #[test]
    fn test_compute_diff_addition() {
        let old = "line1".to_string();
        let new = "line1\nline2".to_string();

        let diff = compute_diff(old, new);

        assert!(diff.contains("line1"));
    }

    #[test]
    fn test_html_escape_all_special_chars() {
        assert_eq!(html_escape("&"), "&amp;");
        assert_eq!(html_escape("<"), "&lt;");
        assert_eq!(html_escape(">"), "&gt;");
        assert_eq!(html_escape("\""), "&quot;");
        assert_eq!(html_escape("'"), "&#39;");
    }

    #[test]
    fn test_html_escape_combined() {
        let input = r#"<div class="test" id='123' data-value="a&b">content</div>"#;
        let escaped = html_escape(input);

        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        assert!(!escaped.contains('"'));
        assert!(!escaped.contains('\''));
        assert!(escaped.contains("&amp;"));
        assert!(escaped.contains("&lt;"));
        assert!(escaped.contains("&gt;"));
    }

    #[test]
    fn test_html_escape_preserves_regular_text() {
        let input = "Regular text with no special chars";
        let escaped = html_escape(input);
        assert_eq!(escaped, input);
    }

    #[test]
    fn test_transform_result_fields() {
        let handle = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "user".to_string(),
            "pass".to_string(),
        )
        .unwrap();

        let rules_json = r#"{"rules":[]}"#;

        let result =
            apply_rules(handle, "Test content".to_string(), rules_json.to_string()).unwrap();

        assert!(!result.new_wikitext.is_empty());
        assert!(!result.summary.is_empty());
        assert!(!result.diff_html.is_empty());
    }

    #[test]
    fn test_apply_rules_with_invalid_json() {
        let handle = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "user".to_string(),
            "pass".to_string(),
        )
        .unwrap();

        let invalid_json = "not valid json";

        let result = apply_rules(handle, "content".to_string(), invalid_json.to_string());

        assert!(result.is_err());
        match result {
            Err(FfiError::ParseError(_)) => (),
            _ => panic!("Expected ParseError for invalid JSON"),
        }
    }

    #[test]
    fn test_ffi_error_display() {
        let err1 = FfiError::NetworkError("connection failed".to_string());
        assert!(err1.to_string().contains("Network error"));

        let err2 = FfiError::AuthenticationError;
        assert_eq!(err2.to_string(), "Authentication failed");

        let err3 = FfiError::NotFound;
        assert_eq!(err3.to_string(), "Resource not found");

        let err4 = FfiError::PermissionDenied;
        assert_eq!(err4.to_string(), "Permission denied");

        let err5 = FfiError::ParseError("invalid".to_string());
        assert!(err5.to_string().contains("Parse error"));

        let err6 = FfiError::SessionNotFound;
        assert_eq!(err6.to_string(), "Session not found");

        let err7 = FfiError::LockPoisoned;
        assert_eq!(err7.to_string(), "Lock poisoned");

        let err8 = FfiError::EngineError("transform failed".to_string());
        assert!(err8.to_string().contains("Engine error"));
    }

    #[test]
    fn test_destroy_session() {
        let handle = create_session(
            "https://en.wikipedia.org/w/api.php".to_string(),
            "user".to_string(),
            "pass".to_string(),
        )
        .unwrap();
        let id = handle.id;
        assert!(destroy_session(SessionHandle { id }).is_ok());
        // Should fail now
        assert!(matches!(
            destroy_session(SessionHandle { id }),
            Err(FfiError::SessionNotFound)
        ));
    }

    // Note: Tests that require actual network calls (login, get_page, save_page)
    // are integration tests and should be run against a test wiki instance.
}
