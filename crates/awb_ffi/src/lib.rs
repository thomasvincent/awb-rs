pub mod c_api;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use awb_domain::types::*;
use awb_domain::rules::RuleSet;
use awb_engine::transform::TransformEngine;
use awb_engine::general_fixes::FixRegistry;
use awb_engine::diff_engine;

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
    password: String,
    // In a real implementation, this would hold the API client
    // For now, we'll use a simple in-memory structure
}

lazy_static::lazy_static! {
    static ref SESSIONS: Arc<Mutex<HashMap<u64, Session>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref NEXT_SESSION_ID: Arc<Mutex<u64>> = Arc::new(Mutex::new(1));
}

// UniFFI exported functions
pub fn create_session(wiki_url: String, username: String, password: String) -> SessionHandle {
    let mut sessions = SESSIONS.lock().unwrap();
    let mut next_id = NEXT_SESSION_ID.lock().unwrap();

    let id = *next_id;
    *next_id += 1;

    sessions.insert(id, Session {
        wiki_url,
        username,
        password,
    });

    SessionHandle { id }
}

pub fn login(handle: SessionHandle) -> Result<(), FfiError> {
    let sessions = SESSIONS.lock().unwrap();
    let _session = sessions
        .get(&handle.id)
        .ok_or(FfiError::InternalError("Invalid session handle".to_string()))?;

    // TODO: Implement actual authentication via awb_mw_api
    // For now, just return success
    Ok(())
}

pub fn fetch_list(
    handle: SessionHandle,
    source: String,
    query: String,
) -> Result<Vec<String>, FfiError> {
    let sessions = SESSIONS.lock().unwrap();
    let _session = sessions
        .get(&handle.id)
        .ok_or(FfiError::InternalError("Invalid session handle".to_string()))?;

    // TODO: Implement actual list fetching via awb_mw_api
    // For now, return a mock list
    Ok(vec![
        format!("Page from {}: {}", source, query),
        "Example Page 1".to_string(),
        "Example Page 2".to_string(),
    ])
}

pub fn get_page(handle: SessionHandle, title: String) -> Result<PageInfo, FfiError> {
    let sessions = SESSIONS.lock().unwrap();
    let _session = sessions
        .get(&handle.id)
        .ok_or(FfiError::InternalError("Invalid session handle".to_string()))?;

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
    let sessions = SESSIONS.lock().unwrap();
    let _session = sessions
        .get(&handle.id)
        .ok_or(FfiError::InternalError("Invalid session handle".to_string()))?;

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
    let sessions = SESSIONS.lock().unwrap();
    let _session = sessions
        .get(&handle.id)
        .ok_or(FfiError::InternalError("Invalid session handle".to_string()))?;

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
                html.push_str(&format!("<span class='delete'>{}</span>", html_escape(text)));
            }
            DiffOp::Insert { text, .. } => {
                html.push_str(&format!("<span class='insert'>{}</span>", html_escape(text)));
            }
            DiffOp::Replace { old_text, new_text, .. } => {
                html.push_str(&format!("<span class='delete'>{}</span>", html_escape(old_text)));
                html.push_str(&format!("<span class='insert'>{}</span>", html_escape(new_text)));
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
        );
        assert!(handle.id > 0);
    }

    #[test]
    fn test_login() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        );
        let result = login(handle);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fetch_list() {
        let handle = create_session(
            "https://en.wikipedia.org".to_string(),
            "testuser".to_string(),
            "testpass".to_string(),
        );
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
        );
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
}
