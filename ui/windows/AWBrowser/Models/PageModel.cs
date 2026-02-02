using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace AWBrowser.Models;

/// <summary>
/// Model representing a wiki page.
/// </summary>
public class PageModel : INotifyPropertyChanged
{
    private ulong _pageId;
    private string _title = string.Empty;
    private ulong _revision;
    private string _timestamp = string.Empty;
    private string _wikitext = string.Empty;
    private ulong _sizeBytes;
    private bool _isRedirect;
    private bool _isModified;

    public event PropertyChangedEventHandler? PropertyChanged;

    public ulong PageId
    {
        get => _pageId;
        set => SetField(ref _pageId, value);
    }

    public string Title
    {
        get => _title;
        set => SetField(ref _title, value);
    }

    public ulong Revision
    {
        get => _revision;
        set => SetField(ref _revision, value);
    }

    public string Timestamp
    {
        get => _timestamp;
        set => SetField(ref _timestamp, value);
    }

    public string Wikitext
    {
        get => _wikitext;
        set
        {
            if (SetField(ref _wikitext, value))
            {
                IsModified = true;
            }
        }
    }

    public ulong SizeBytes
    {
        get => _sizeBytes;
        set => SetField(ref _sizeBytes, value);
    }

    public bool IsRedirect
    {
        get => _isRedirect;
        set => SetField(ref _isRedirect, value);
    }

    public bool IsModified
    {
        get => _isModified;
        set => SetField(ref _isModified, value);
    }

    protected void OnPropertyChanged([CallerMemberName] string? propertyName = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }

    protected bool SetField<T>(ref T field, T value, [CallerMemberName] string? propertyName = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value)) return false;
        field = value;
        OnPropertyChanged(propertyName);
        return true;
    }
}
