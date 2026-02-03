// GTK4 Linux UI for AWB-RS
//
// This crate provides a native Linux interface using GTK4 and libadwaita.
// It directly uses awb_engine, awb_mw_api, and other Rust crates (no FFI needed).

#[cfg(target_os = "linux")]
pub mod app;
#[cfg(target_os = "linux")]
pub mod views;

// Re-export main application
#[cfg(target_os = "linux")]
pub use app::AwbApplication;
