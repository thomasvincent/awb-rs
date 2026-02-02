using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using AWBrowser.Models;

namespace AWBrowser.ViewModels;

/// <summary>
/// View model for the main session state.
/// Implements MVVM pattern for data binding.
/// </summary>
public class SessionViewModel : INotifyPropertyChanged
{
    private string _wikiUrl = string.Empty;
    private string _username = string.Empty;
    private bool _isLoggedIn;
    private string _statusMessage = "Ready";
    private int _progressValue;

    public event PropertyChangedEventHandler? PropertyChanged;

    public SessionViewModel()
    {
        Pages = new ObservableCollection<PageModel>();
        Rules = new ObservableCollection<RuleModel>();
    }

    // ============================================================================
    // Properties
    // ============================================================================

    public string WikiUrl
    {
        get => _wikiUrl;
        set => SetField(ref _wikiUrl, value);
    }

    public string Username
    {
        get => _username;
        set => SetField(ref _username, value);
    }

    public bool IsLoggedIn
    {
        get => _isLoggedIn;
        set => SetField(ref _isLoggedIn, value);
    }

    public string StatusMessage
    {
        get => _statusMessage;
        set => SetField(ref _statusMessage, value);
    }

    public int ProgressValue
    {
        get => _progressValue;
        set => SetField(ref _progressValue, value);
    }

    public ObservableCollection<PageModel> Pages { get; }
    public ObservableCollection<RuleModel> Rules { get; }

    // ============================================================================
    // INotifyPropertyChanged Implementation
    // ============================================================================

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
