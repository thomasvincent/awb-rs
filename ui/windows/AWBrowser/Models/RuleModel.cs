using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace AWBrowser.Models;

/// <summary>
/// Model representing a find/replace rule.
/// </summary>
public class RuleModel : INotifyPropertyChanged
{
    private bool _enabled = true;
    private string _name = string.Empty;
    private string _pattern = string.Empty;
    private string _replacement = string.Empty;
    private bool _isRegex;
    private bool _caseSensitive;
    private string _description = string.Empty;

    public event PropertyChangedEventHandler? PropertyChanged;

    public bool Enabled
    {
        get => _enabled;
        set => SetField(ref _enabled, value);
    }

    public string Name
    {
        get => _name;
        set => SetField(ref _name, value);
    }

    public string Pattern
    {
        get => _pattern;
        set => SetField(ref _pattern, value);
    }

    public string Replacement
    {
        get => _replacement;
        set => SetField(ref _replacement, value);
    }

    public bool IsRegex
    {
        get => _isRegex;
        set => SetField(ref _isRegex, value);
    }

    public bool CaseSensitive
    {
        get => _caseSensitive;
        set => SetField(ref _caseSensitive, value);
    }

    public string Description
    {
        get => _description;
        set => SetField(ref _description, value);
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
