using System.Windows.Controls;

namespace AWBrowser.Views;

/// <summary>
/// Interaction logic for EditorView.xaml
/// Split diff view showing before/after comparison.
/// </summary>
public partial class EditorView : UserControl
{
    public EditorView()
    {
        InitializeComponent();
    }

    public void SetDiff(string beforeText, string afterText)
    {
        BeforeTextBox.Text = beforeText;
        AfterTextBox.Text = afterText;
    }

    public void Clear()
    {
        BeforeTextBox.Clear();
        AfterTextBox.Clear();
    }
}
