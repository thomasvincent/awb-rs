use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Failed to load plugin: {0}")]
    LoadFailed(String),

    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Plugin returned invalid data: {0}")]
    InvalidReturn(String),

    #[error("Plugin execution timed out after {0}s")]
    Timeout(u64),

    #[error("Sandboxing violation: {0}")]
    Sandboxed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("WASM error: {0}")]
    Wasm(#[from] wasmtime::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

pub type Result<T> = std::result::Result<T, PluginError>;
