using KuroShell.Core;
using Microsoft.UI.Xaml.Controls;

namespace KuroShell.Pages;

public static class PageRegistry
{
    private static readonly PageCatalog Catalog = PageCatalog.CreateDefault();

    private static readonly Dictionary<string, Func<UserControl>> Factories = new(StringComparer.Ordinal)
    {
        [PageIds.Settings] = static () => new Settings.SettingsPage(),
        [PageIds.Health] = static () => new Health.HealthPage(),
        [PageIds.Phrases] = static () => new Phrases.PhrasesPage(),
    };

    public static void Register(IPageDescriptor descriptor, Func<UserControl> createView)
    {
        ArgumentNullException.ThrowIfNull(descriptor);
        ArgumentNullException.ThrowIfNull(createView);
        Catalog.Register(descriptor);
        Factories[descriptor.Id] = createView;
    }

    public static IPageDescriptor Resolve(string pageId) => Catalog.Resolve(pageId);

    public static UserControl ResolveView(string pageId)
    {
        var descriptor = Catalog.Resolve(pageId);
        if (!Factories.TryGetValue(descriptor.Id, out var create))
        {
            throw new InvalidOperationException($"Shell page '{descriptor.Id}' is registered without a view factory.");
        }

        return create();
    }
}