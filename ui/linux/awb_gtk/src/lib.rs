// GTK4 Linux UI for AWB-RS
//
// This crate provides a native Linux interface using GTK4 and libadwaita.
// It interfaces with the Rust FFI layer (awb_ffi) for all wiki operations.

pub mod app;
pub mod views;

// Re-export main application
pub use app::AwbApplication;
