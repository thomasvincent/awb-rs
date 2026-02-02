use awb_domain::types::RevisionId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MwApiError {
    #[error("HTTP error: {status} {url}")]
    Http { status: u16, url: String, body: String },

    #[error("maxlag exceeded: retry after {retry_after}s")]
    MaxLag { retry_after: u64 },

    #[error("Rate limited (429): retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    #[error("Service unavailable (503)")]
    ServiceUnavailable,

    #[error("Edit conflict: base={base_rev:?}, current={current_rev:?}")]
    EditConflict { base_rev: RevisionId, current_rev: RevisionId },

    #[error("Token expired, refresh needed")]
    BadToken,

    #[error("API error: {code} — {info}")]
    ApiError { code: String, info: String },

    #[error("Auth failed: {reason}")]
    AuthError { reason: String },

    #[error("Deserialization: {0}")]
    Deserialize(#[from] serde_json::Error),

    #[error("Network: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Timeout after {0:?}")]
    Timeout(std::time::Duration),
}

impl MwApiError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::MaxLag { .. } | Self::RateLimited { .. } | Self::ServiceUnavailable | Self::BadToken | Self::Network(_))
    }
}
