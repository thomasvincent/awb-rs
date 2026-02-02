using System.Windows;
using System.Windows.Controls;
using AWBrowser.Models;

namespace AWBrowser.Views;

/// <summary>
/// Interaction logic for RuleEditorView.xaml
/// </summary>
public partial class RuleEditorView : UserControl
{
    public RuleEditorView()
    {
        InitializeComponent();

        // Initialize with empty rule list
        RulesDataGrid.ItemsSource = new System.Collections.ObjectModel.ObservableCollection<RuleModel>();
    }

    private void NewRule_Click(object sender, RoutedEventArgs e)
    {
        MessageBox.Show("New rule functionality to be implemented", "Not Implemented");
    }

    private void LoadRules_Click(object sender, RoutedEventArgs e)
    {
        MessageBox.Show("Load rules functionality to be implemented", "Not Implemented");
    }

    private void SaveRules_Click(object sender, RoutedEventArgs e)
    {
        MessageBox.Show("Save rules functionality to be implemented", "Not Implemented");
    }

    private void ImportRules_Click(object sender, RoutedEventArgs e)
    {
        MessageBox.Show("Import rules functionality to be implemented", "Not Implemented");
    }

    private void ExportRules_Click(object sender, RoutedEventArgs e)
    {
        MessageBox.Show("Export rules functionality to be implemented", "Not Implemented");
    }

    private void MoveUp_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Move selected rule up in list
    }

    private void MoveDown_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Move selected rule down in list
    }

    private void TestRule_Click(object sender, RoutedEventArgs e)
    {
        MessageBox.Show("Test rule functionality to be implemented", "Not Implemented");
    }

    private void DeleteRule_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Delete selected rule
    }
}
