using System.Windows;
using System.Windows.Input;
using AWBrowser.ViewModels;
using AWBrowser.Views;

namespace AWBrowser;

/// <summary>
/// Interaction logic for MainWindow.xaml
/// </summary>
public partial class MainWindow : Window
{
    private SessionViewModel? _viewModel;

    public MainWindow()
    {
        InitializeComponent();

        // Initialize view model
        _viewModel = new SessionViewModel();
        DataContext = _viewModel;
    }

    // ============================================================================
    // Menu Handlers
    // ============================================================================

    private void Login_Click(object sender, RoutedEventArgs e)
    {
        var loginWindow = new LoginWindow
        {
            Owner = this
        };

        if (loginWindow.ShowDialog() == true)
        {
            // TODO: Create session and login using FFI
            StatusText.Text = $"Logged in as {loginWindow.Username} to {loginWindow.WikiUrl}";
        }
    }

    private void Open_Executed(object sender, ExecutedRoutedEventArgs e)
    {
        // TODO: Open page dialog
        MessageBox.Show("Open page functionality to be implemented", "Not Implemented",
            MessageBoxButton.OK, MessageBoxImage.Information);
    }

    private void Save_Executed(object sender, ExecutedRoutedEventArgs e)
    {
        // TODO: Save page via FFI
        MessageBox.Show("Save page functionality to be implemented", "Not Implemented",
            MessageBoxButton.OK, MessageBoxImage.Information);
    }

    private void Close_Executed(object sender, ExecutedRoutedEventArgs e)
    {
        Close();
    }

    private void ToggleDiff_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Toggle diff view visibility
    }

    private void ToggleRules_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Toggle rules panel visibility
    }

    private void ApplyRules_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Apply rules via FFI
        MessageBox.Show("Apply rules functionality to be implemented", "Not Implemented",
            MessageBoxButton.OK, MessageBoxImage.Information);
    }

    private void PreviewChanges_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Generate diff preview
        MessageBox.Show("Preview functionality to be implemented", "Not Implemented",
            MessageBoxButton.OK, MessageBoxImage.Information);
    }

    private void Settings_Click(object sender, RoutedEventArgs e)
    {
        MessageBox.Show("Settings dialog to be implemented", "Not Implemented",
            MessageBoxButton.OK, MessageBoxImage.Information);
    }

    private void About_Click(object sender, RoutedEventArgs e)
    {
        string version = NativeMethods.GetVersion();
        MessageBox.Show(
            $"AWBrowser - AutoWikiBrowser Rust Edition\n\n" +
            $"Version: {version}\n" +
            $"License: MIT OR Apache-2.0\n\n" +
            $"A modern WikiText editor built with Rust and C#/WPF",
            "About AWBrowser",
            MessageBoxButton.OK,
            MessageBoxImage.Information);
    }

    // ============================================================================
    // Toolbar Handlers
    // ============================================================================

    private void FetchList_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Show fetch list dialog
        MessageBox.Show("Fetch list functionality to be implemented", "Not Implemented",
            MessageBoxButton.OK, MessageBoxImage.Information);
    }

    private void GetPage_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Get single page
        MessageBox.Show("Get page functionality to be implemented", "Not Implemented",
            MessageBoxButton.OK, MessageBoxImage.Information);
    }

    // ============================================================================
    // Page List Handlers
    // ============================================================================

    private void PageListBox_SelectionChanged(object sender, System.Windows.Controls.SelectionChangedEventArgs e)
    {
        // TODO: Load selected page into editor
        if (PageListBox.SelectedItem != null)
        {
            StatusText.Text = $"Loaded: {PageListBox.SelectedItem}";
        }
    }

    // ============================================================================
    // Rules Handlers
    // ============================================================================

    private void AddRule_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Add new rule to grid
    }

    private void RemoveRule_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Remove selected rule
    }

    private void MoveRuleUp_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Move selected rule up
    }

    private void MoveRuleDown_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Move selected rule down
    }
}
