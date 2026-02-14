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
    // Session Management
    // ============================================================================

    /// <summary>
    /// Creates a new session handle.
    /// Returns opaque result that must be checked with awb_result_is_ok().
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr create_session(string wiki_url, string username, string password);

    /// <summary>
    /// Destroys a session handle and releases associated resources.
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr destroy_session(SessionHandle handle);

    /// <summary>
    /// Logs in the session with stored credentials.
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr login(SessionHandle handle);

    /// <summary>
    /// Fetches a list of page titles from the specified source and query.
    /// Returns opaque result containing Vec<String>.
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr fetch_list(SessionHandle handle, string source, string query);

    /// <summary>
    /// Retrieves page information for the specified title.
    /// Returns opaque result containing PageInfo.
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr get_page(SessionHandle handle, string title);

    /// <summary>
    /// Saves a page with the specified content and summary.
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr save_page(SessionHandle handle, string title, string content, string summary);

    /// <summary>
    /// Applies rules/transformations to content.
    /// Returns opaque result containing TransformResult.
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr apply_rules(SessionHandle handle, string content, string rules_json);

    /// <summary>
    /// Computes a diff between old and new text, returns HTML formatted diff.
    /// Caller must free with awb_free_string().
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr compute_diff(string old_text, string new_text);

    // ============================================================================
    // Memory Management
    // ============================================================================

    /// <summary>
    /// Frees a PageInfo struct returned by get_page().
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void awb_free_page_info(IntPtr ptr);

    /// <summary>
    /// Frees a TransformResult struct returned by apply_rules().
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void awb_free_transform_result(IntPtr ptr);

    /// <summary>
    /// Frees a Vec<String> returned by fetch_list().
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void awb_free_string_vec(IntPtr ptr);

    // ============================================================================
    // Helper Methods for Safe Marshaling
    // ============================================================================

    /// <summary>
    /// Safely executes a session operation and returns a result.
    /// </summary>
    public static T ExecuteSessionOperation<T>(
        Func<IntPtr> operation,
        Func<IntPtr, T> marshal,
        Action<IntPtr>? cleanup = null)
    {
        IntPtr result = operation();
        if (result == IntPtr.Zero)
        {
            throw new InvalidOperationException("FFI operation returned null");
        }

        try
        {
            return marshal(result);
        }
        finally
        {
            cleanup?.Invoke(result);
        }
    }

    /// <summary>
    /// Gets a PageInfo from the opaque pointer returned by get_page().
    /// </summary>
    public static PageInfo MarshalPageInfo(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
        {
            throw new InvalidOperationException("Cannot marshal null PageInfo pointer");
        }

        return Marshal.PtrToStructure<PageInfo>(ptr)
            ?? throw new InvalidOperationException("Failed to marshal PageInfo");
    }

    /// <summary>
    /// Gets a TransformResult from the opaque pointer returned by apply_rules().
    /// Note: This is a simplified version - full implementation requires proper serialization.
    /// </summary>
    public static TransformResult MarshalTransformResult(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
        {
            throw new InvalidOperationException("Cannot marshal null TransformResult pointer");
        }

        // For complex types with String arrays, consider using JSON serialization
        // or UniFFI-generated bindings instead of raw P/Invoke.
        return new TransformResult();
    }

    /// <summary>
    /// Gets a string list from the opaque pointer returned by fetch_list().
    /// Note: Requires proper serialization from Rust Vec<String>.
    /// </summary>
    public static string[] MarshalStringVec(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
        {
            throw new InvalidOperationException("Cannot marshal null string vector pointer");
        }

        // TODO: Implement proper Vec<String> marshaling
        // This may require additional C API helpers to iterate the vector
        return Array.Empty<string>();
    }
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
