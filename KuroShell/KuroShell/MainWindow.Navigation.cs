using KuroShell.Core;
using KuroShell.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace KuroShell;

public sealed partial class MainWindow
{
    public async Task ShowPage(string page)
    {
        page = ShellNavigation.NormalizePage(page);
        var registry = ShellNavigation.IsRegistryPage(page);
        ConversationPage.Visibility = !registry && page == "Conversation" ? Visibility.Visible : Visibility.Collapsed;
        VoicePage.Visibility = !registry && page == "Voice" ? Visibility.Visible : Visibility.Collapsed;
        DevicesPage.Visibility = !registry && page == "Devices" ? Visibility.Visible : Visibility.Collapsed;
        ContactsPage.Visibility = !registry && page == "Contacts" ? Visibility.Visible : Visibility.Collapsed;
        SystemPage.Visibility = !registry && page == "System" ? Visibility.Visible : Visibility.Collapsed;
        RegistryHost.Visibility = registry ? Visibility.Visible : Visibility.Collapsed;
        if (registry)
        {
            RegistryHost.Content = PageRegistry.ResolveView(page, _client);
        }

        Select(page);
        try
        {
            if (!registry)
            {
                await Fill(page);
            }

            SetStatus("");
        }
        catch (Exception)
        {
            SetStatus("The engine API is not answering.");
        }
    }

    private void OnNav(object sender, RoutedEventArgs e)
    {
        if (sender is Button button && button.Tag is string page)
        {
            _ = ShowPage(page);
        }
    }

    private async Task Fill(string page)
    {
        switch (page)
        {
            case "Voice":
                await ShowVoice();
                break;
            case "Devices":
                await ShowDevices();
                break;
            case "Contacts":
                await ShowContacts();
                break;
            case "System":
                await ShowSystem();
                break;
        }
    }
}