using System;
using System.Runtime.InteropServices;

namespace AWBrowser;

/// <summary>
/// P/Invoke declarations for the AWB FFI C API.
/// Maps to functions in awb_ffi (crates/awb_ffi/src/c_api.rs).
/// </summary>
public static class NativeMethods
{
    private const string LibName = "awb_ffi";

    // ============================================================================
    // Version and Utility Functions
    // ============================================================================

    /// <summary>
    /// Returns the library version string.
    /// Caller must free with awb_free_string().
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr awb_version();

    /// <summary>
    /// Frees a string previously returned by awb_version() or other C API functions.
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void awb_free_string(IntPtr ptr);

    // ============================================================================
    // Helper Methods for String Marshaling
    // ============================================================================

    /// <summary>
    /// Gets the version string from the native library.
    /// </summary>
    public static string GetVersion()
    {
        IntPtr ptr = awb_version();
        try
        {
            return Marshal.PtrToStringAnsi(ptr) ?? "unknown";
        }
        finally
        {
            awb_free_string(ptr);
        }
    }

    // ============================================================================
    // Session Management (to be implemented)
    // ============================================================================
    // The following functions would map to the Rust FFI in lib.rs:
    // - create_session(wiki_url, username, password) -> SessionHandle
    // - login(handle) -> Result
    // - fetch_list(handle, source, query) -> Vec<String>
    // - get_page(handle, title) -> PageInfo
    // - apply_rules(handle, content, rules_json) -> TransformResult
    // - save_page(handle, title, content, summary) -> Result
    // - compute_diff(old_text, new_text) -> String
    //
    // For UniFFI-based bindings, consider using the generated C# bindings
    // from UniFFI instead of manual P/Invoke declarations.
    // This file shows the pattern for direct C API access.
}

/// <summary>
/// Represents a session handle returned from create_session.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
public struct SessionHandle
{
    public ulong Id;
}

/// <summary>
/// Represents page information returned from get_page.
/// </summary>
public class PageInfo
{
    public ulong PageId { get; set; }
    public string Title { get; set; } = string.Empty;
    public ulong Revision { get; set; }
    public string Timestamp { get; set; } = string.Empty;
    public string Wikitext { get; set; } = string.Empty;
    public ulong SizeBytes { get; set; }
    public bool IsRedirect { get; set; }
}

/// <summary>
/// Represents the result of applying rules/transformations.
/// </summary>
public class TransformResult
{
    public string NewWikitext { get; set; } = string.Empty;
    public string[] RulesApplied { get; set; } = Array.Empty<string>();
    public string[] FixesApplied { get; set; } = Array.Empty<string>();
    public string Summary { get; set; } = string.Empty;
    public string[] Warnings { get; set; } = Array.Empty<string>();
    public string DiffHtml { get; set; } = string.Empty;
}
