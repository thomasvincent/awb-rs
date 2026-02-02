pub mod c_api;

use awb_domain::rules::RuleSet;
use awb_domain::types::*;
use awb_engine::diff_engine;
use awb_engine::general_fixes::FixRegistry;
use awb_engine::transform::TransformEngine;
use secrecy::SecretString;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    #[error("Internal error: {0}")]
    InternalError(String),
}

// Session storage
struct Session {
    wiki_url: String,
    username: String,
    password: Option<SecretString>,
    // In a real implementation, this would hold the API client
    // For now, we'll use a simple in-memory structure
}

lazy_static::lazy_static! {
    static ref SESSIONS: Arc<Mutex<HashMap<u64, Session>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref NEXT_SESSION_ID: Arc<Mutex<u64>> = Arc::new(Mutex::new(1));
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

    let mut sessions = SESSIONS
        .lock()
        .map_err(|_| FfiError::InternalError("Session lock poisoned".to_string()))?;
    let mut next_id = NEXT_SESSION_ID
        .lock()
        .map_err(|_| FfiError::InternalError("Session ID lock poisoned".to_string()))?;

    let id = *next_id;
    *next_id += 1;

    sessions.insert(
        id,
        Session {
            wiki_url,
            username,
            password: Some(SecretString::new(password.into())),
        },
    );

    Ok(SessionHandle { id })
}

pub fn login(handle: SessionHandle) -> Result<(), FfiError> {
    let mut sessions = SESSIONS
        .lock()
        .map_err(|_| FfiError::InternalError("Session lock poisoned".to_string()))?;
    let session = sessions.get_mut(&handle.id).ok_or(FfiError::InternalError(
        "Invalid session handle".to_string(),
    ))?;

    // TODO: Implement actual authentication via awb_mw_api
    // Use password here: let _password = session.password.as_ref().map(|p| p.expose_secret());

    // Clear password after authentication completes
    session.password = None;

    Ok(())
}

pub fn fetch_list(
    handle: SessionHandle,
    source: String,
    query: String,
) -> Result<Vec<String>, FfiError> {
    let sessions = SESSIONS
        .lock()
        .map_err(|_| FfiError::InternalError("Session lock poisoned".to_string()))?;
    let _session = sessions.get(&handle.id).ok_or(FfiError::InternalError(
        "Invalid session handle".to_string(),
    ))?;

    // TODO: Implement actual list fetching via awb_mw_api
    // For now, return a mock list
    Ok(vec![
        format!("Page from {}: {}", source, query),
        "Example Page 1".to_string(),
        "Example Page 2".to_string(),
    ])
}

pub fn get_page(handle: SessionHandle, title: String) -> Result<PageInfo, FfiError> {
    let sessions = SESSIONS
        .lock()
        .map_err(|_| FfiError::InternalError("Session lock poisoned".to_string()))?;
    let _session = sessions.get(&handle.id).ok_or(FfiError::InternalError(
        "Invalid session handle".to_string(),
    ))?;

    // TODO: Implement actual page fetching via awb_mw_api
    // For now, return a mock page
    Ok(PageInfo {
        page_id: 12345,
        title: title.clone(),
        revision: 98765,
        timestamp: chrono::Utc::now().to_rfc3339(),
        wikitext: format!("This is the content of [[{}]]\n\nSome example text.", title),
        size_bytes: 100,
        is_redirect: false,
    })
}

pub fn apply_rules(
    handle: SessionHandle,
    content: String,
    rules_json: String,
) -> Result<TransformResult, FfiError> {
    let sessions = SESSIONS
        .lock()
        .map_err(|_| FfiError::InternalError("Session lock poisoned".to_string()))?;
    let _session = sessions.get(&handle.id).ok_or(FfiError::InternalError(
        "Invalid session handle".to_string(),
    ))?;

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
        .map_err(|e| FfiError::InternalError(format!("Transform engine error: {}", e)))?;

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
    let sessions = SESSIONS
        .lock()
        .map_err(|_| FfiError::InternalError("Session lock poisoned".to_string()))?;
    let _session = sessions.get(&handle.id).ok_or(FfiError::InternalError(
        "Invalid session handle".to_string(),
    ))?;

    // TODO: Implement actual page saving via awb_mw_api
    // For now, just validate inputs
    if title.is_empty() {
        return Err(FfiError::ParseError("Title cannot be empty".to_string()));
    }
    if content.is_empty() {
        return Err(FfiError::ParseError("Content cannot be empty".to_string()));
    }
    if summary.is_empty() {
        return Err(FfiError::ParseError("Summary cannot be empty".to_string()));
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
            "https://en.wikipedia.org".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        )
        .unwrap();
        assert!(handle.id > 0);
    }

    #[test]
    fn test_login() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        )
        .unwrap();
        let result = login(handle);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fetch_list() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        )
        .unwrap();
        let result = fetch_list(handle, "category".to_string(), "Test".to_string());
        assert!(result.is_ok());
        let list = result.unwrap();
        assert!(!list.is_empty());
    }

    #[test]
    fn test_get_page() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        )
        .unwrap();
        let result = get_page(handle, "Test Page".to_string());
        assert!(result.is_ok());
        let page = result.unwrap();
        assert_eq!(page.title, "Test Page");
        assert!(!page.wikitext.is_empty());
    }

    #[test]
    fn test_compute_diff() {
        let old = "hello world".to_string();
        let new = "hello there world".to_string();
        let diff = compute_diff(old, new);
        assert!(diff.contains("hello"));
        assert!(diff.contains("world"));
    }

    #[test]
    fn test_html_escape() {
        let input = "<script>alert('xss')</script>";
        let escaped = html_escape(input);
        assert_eq!(escaped, "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;");
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
            "https://en.wikipedia.org".to_string(),
            "user1".to_string(),
            "pass1".to_string(),
        )
        .unwrap();

        let handle2 = create_session(
            "https://en.wikipedia.org".to_string(),
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
    fn test_create_session_with_empty_username() {
        let result = create_session(
            "https://en.wikipedia.org".to_string(),
            "".to_string(),
            "pass".to_string(),
        );

        // Empty username should still create session
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_session_with_empty_password() {
        let result = create_session(
            "https://en.wikipedia.org".to_string(),
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
    fn test_login_with_valid_handle() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        )
        .unwrap();

        let result = login(handle);
        assert!(result.is_ok(), "Login should succeed with valid handle");
    }

    #[test]
    fn test_save_page_validates_title() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
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
            "https://en.wikipedia.org".to_string(),
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
            "https://en.wikipedia.org".to_string(),
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
    fn test_save_page_with_valid_inputs() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
            "user".to_string(),
            "pass".to_string(),
        )
        .unwrap();

        let result = save_page(
            handle,
            "Test Page".to_string(),
            "Test content".to_string(),
            "Test summary".to_string(),
        );

        assert!(result.is_ok(), "Should succeed with valid inputs");
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
        // Should contain insert markup for line2
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
    fn test_page_info_fields() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
            "user".to_string(),
            "pass".to_string(),
        )
        .unwrap();

        let page = get_page(handle, "Test".to_string()).unwrap();

        assert_eq!(page.title, "Test");
        assert!(page.page_id > 0);
        assert!(page.revision > 0);
        assert!(!page.timestamp.is_empty());
        assert!(!page.wikitext.is_empty());
        assert!(page.size_bytes > 0);
        assert!(!page.is_redirect);
    }

    #[test]
    fn test_transform_result_fields() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
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
            "https://en.wikipedia.org".to_string(),
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

        let err6 = FfiError::InternalError("bug".to_string());
        assert!(err6.to_string().contains("Internal error"));
    }
}
