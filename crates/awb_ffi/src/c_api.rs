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
///
/// # Safety
///
/// The caller must ensure that:
/// - `ptr` is either null or was previously returned by `awb_version()`
/// - `ptr` has not been freed or modified since it was returned
/// - This function is only called once per pointer
#[unsafe(no_mangle)]
pub unsafe extern "C" fn awb_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

// ============================================================================
// Session Management API
// ============================================================================

use crate::{create_session as ffi_create_session, destroy_session as ffi_destroy_session};
use crate::{login as ffi_login, get_page as ffi_get_page, save_page as ffi_save_page};
use crate::{fetch_list as ffi_fetch_list, apply_rules as ffi_apply_rules, compute_diff as ffi_compute_diff};
use crate::{SessionHandle, PageInfo, TransformResult};
use std::ffi::CStr;

/// Creates a new session handle.
/// Returns an opaque result pointer that should be checked with error handling.
///
/// # Safety
/// Caller must ensure wiki_url, username, and password are valid UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn create_session(
    wiki_url: *const c_char,
    username: *const c_char,
    password: *const c_char,
) -> *mut SessionHandle {
    if wiki_url.is_null() || username.is_null() || password.is_null() {
        return std::ptr::null_mut();
    }

    let wiki_url_str = match CStr::from_ptr(wiki_url).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };

    let username_str = match CStr::from_ptr(username).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };

    let password_str = match CStr::from_ptr(password).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };

    match ffi_create_session(wiki_url_str, username_str, password_str) {
        Ok(handle) => Box::into_raw(Box::new(handle)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Destroys a session handle and releases resources.
#[unsafe(no_mangle)]
pub extern "C" fn destroy_session(handle: SessionHandle) -> i32 {
    match ffi_destroy_session(handle) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Logs in with the session's stored credentials.
#[unsafe(no_mangle)]
pub extern "C" fn login(handle: SessionHandle) -> i32 {
    match ffi_login(handle) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Fetches a list of pages matching the source and query.
/// Returns an opaque result pointer.
///
/// # Safety
/// Caller must ensure source and query are valid UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fetch_list(
    handle: SessionHandle,
    source: *const c_char,
    query: *const c_char,
) -> *mut Vec<String> {
    if source.is_null() || query.is_null() {
        return std::ptr::null_mut();
    }

    let source_str = match CStr::from_ptr(source).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };

    let query_str = match CStr::from_ptr(query).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };

    match ffi_fetch_list(handle, source_str, query_str) {
        Ok(list) => Box::into_raw(Box::new(list)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Retrieves page information for the specified title.
/// Returns an opaque result pointer.
///
/// # Safety
/// Caller must ensure title is a valid UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_page(
    handle: SessionHandle,
    title: *const c_char,
) -> *mut PageInfo {
    if title.is_null() {
        return std::ptr::null_mut();
    }

    let title_str = match CStr::from_ptr(title).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };

    match ffi_get_page(handle, title_str) {
        Ok(page_info) => Box::into_raw(Box::new(page_info)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Saves a page with the specified content and summary.
///
/// # Safety
/// Caller must ensure all string parameters are valid UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn save_page(
    handle: SessionHandle,
    title: *const c_char,
    content: *const c_char,
    summary: *const c_char,
) -> i32 {
    if title.is_null() || content.is_null() || summary.is_null() {
        return -1;
    }

    let title_str = match CStr::from_ptr(title).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };

    let content_str = match CStr::from_ptr(content).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };

    let summary_str = match CStr::from_ptr(summary).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };

    match ffi_save_page(handle, title_str, content_str, summary_str) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Applies rules/transformations to content.
/// Returns an opaque result pointer.
///
/// # Safety
/// Caller must ensure content and rules_json are valid UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn apply_rules(
    handle: SessionHandle,
    content: *const c_char,
    rules_json: *const c_char,
) -> *mut TransformResult {
    if content.is_null() || rules_json.is_null() {
        return std::ptr::null_mut();
    }

    let content_str = match CStr::from_ptr(content).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };

    let rules_json_str = match CStr::from_ptr(rules_json).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };

    match ffi_apply_rules(handle, content_str, rules_json_str) {
        Ok(result) => Box::into_raw(Box::new(result)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Computes a diff between old and new text, returning HTML formatted diff.
/// Caller must free with awb_free_string().
///
/// # Safety
/// Caller must ensure old_text and new_text are valid UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compute_diff(
    old_text: *const c_char,
    new_text: *const c_char,
) -> *const c_char {
    if old_text.is_null() || new_text.is_null() {
        return std::ptr::null();
    }

    let old_str = match CStr::from_ptr(old_text).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null(),
    };

    let new_str = match CStr::from_ptr(new_text).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null(),
    };

    let diff_html = ffi_compute_diff(old_str, new_str);
    let c_string = match CString::new(diff_html) {
        Ok(s) => s,
        Err(_) => return std::ptr::null(),
    };
    c_string.into_raw()
}

/// Frees a PageInfo struct returned by get_page().
///
/// # Safety
///
/// - `ptr` must be a valid pointer returned by `awb_get_page`, or null.
/// - The pointer must not have been freed previously.
/// - After calling this function, the pointer is invalid and must not be used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn awb_free_page_info(ptr: *mut PageInfo) {
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr);
    }
}

/// Frees a TransformResult struct returned by apply_rules().
///
/// # Safety
///
/// - `ptr` must be a valid pointer returned by `awb_apply_rules`, or null.
/// - The pointer must not have been freed previously.
/// - After calling this function, the pointer is invalid and must not be used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn awb_free_transform_result(ptr: *mut TransformResult) {
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr);
    }
}

/// Frees a Vec<String> returned by fetch_list().
///
/// # Safety
///
/// - `ptr` must be a valid pointer returned by `awb_fetch_list`, or null.
/// - The pointer must not have been freed previously.
/// - After calling this function, the pointer is invalid and must not be used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn awb_free_string_vec(ptr: *mut Vec<String>) {
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr);
    }
}
