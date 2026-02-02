using System.Windows;

namespace AWBrowser.Views;

/// <summary>
/// Interaction logic for LoginWindow.xaml
/// </summary>
public partial class LoginWindow : Window
{
    public string WikiUrl => WikiUrlTextBox.Text;
    public string Username => UsernameTextBox.Text;
    public string Password => PasswordBox.Password;

    public LoginWindow()
    {
        InitializeComponent();
    }

    private void Login_Click(object sender, RoutedEventArgs e)
    {
        // Validate inputs
        if (string.IsNullOrWhiteSpace(WikiUrl))
        {
            MessageBox.Show("Wiki URL is required", "Validation Error",
                MessageBoxButton.OK, MessageBoxImage.Warning);
            WikiUrlTextBox.Focus();
            return;
        }

        if (string.IsNullOrWhiteSpace(Username))
        {
            MessageBox.Show("Username is required", "Validation Error",
                MessageBoxButton.OK, MessageBoxImage.Warning);
            UsernameTextBox.Focus();
            return;
        }

        if (string.IsNullOrWhiteSpace(Password))
        {
            MessageBox.Show("Password is required", "Validation Error",
                MessageBoxButton.OK, MessageBoxImage.Warning);
            PasswordBox.Focus();
            return;
        }

        DialogResult = true;
        Close();
    }

    private void Cancel_Click(object sender, RoutedEventArgs e)
    {
        DialogResult = false;
        Close();
    }
}
