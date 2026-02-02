// C API implementation for platforms that need direct C ABI access
// This complements the UniFFI interface for Swift/Kotlin
//
// The UniFFI-generated bindings in lib.rs are the primary interface for macOS/iOS.
// This file can be extended for additional C-specific functionality if needed.

use std::ffi::CString;
use std::os::raw::c_char;

// Re-export the main FFI functions for C compatibility
// UniFFI handles the actual implementation, but we provide C wrappers here

/// Returns the library version string. Caller must free with awb_free_string().
#[unsafe(no_mangle)]
pub extern "C" fn awb_version() -> *const c_char {
    let version = CString::new(env!("CARGO_PKG_VERSION")).expect("version has no null bytes");
    version.into_raw()
}

/// Frees a string previously returned by awb_version() or other C API functions.
#[unsafe(no_mangle)]
pub extern "C" fn awb_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

// Additional C API functions can be added here as needed
// For now, the UniFFI interface in lib.rs provides the main functionality
