using System.Windows;

namespace AWBrowser;

/// <summary>
/// Interaction logic for App.xaml
/// </summary>
public partial class App : Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // Verify native library is available
        try
        {
            string version = NativeMethods.GetVersion();
            System.Diagnostics.Debug.WriteLine($"AWB FFI Version: {version}");
        }
        catch (Exception ex)
        {
            MessageBox.Show(
                $"Failed to load AWB native library:\n{ex.Message}\n\n" +
                "Make sure awb_ffi.dll is built and present in the application directory.",
                "Initialization Error",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            Shutdown(1);
        }
    }
}
