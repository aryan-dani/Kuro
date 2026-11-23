using KuroShell.Core;
using Microsoft.UI.Xaml.Controls;

namespace KuroShell.Pages;

public static class PageRegistry
{
    private static readonly PageCatalog Catalog = PageCatalog.CreateDefault();

    public static void Register(IPageDescriptor descriptor, Func<EngineClient?, UserControl> createView)
    {
        ArgumentNullException.ThrowIfNull(descriptor);
        ArgumentNullException.ThrowIfNull(createView);
        Catalog.Register(descriptor);
        Factories[descriptor.Id] = createView;
    }

    private static readonly Dictionary<string, Func<EngineClient?, UserControl>> Factories = new(StringComparer.Ordinal)
    {
        [PageIds.Settings] = static client => new Settings.SettingsPage(client),
        [PageIds.Health] = static client => new Health.HealthPage(client),
        [PageIds.Phrases] = static client => new Phrases.PhrasesPage(client),
    };

    public static IPageDescriptor Resolve(string pageId) => Catalog.Resolve(pageId);

    public static UserControl ResolveView(string pageId, EngineClient? client = null)
    {
        var descriptor = Catalog.Resolve(pageId);
        if (!Factories.TryGetValue(descriptor.Id, out var create))
        {
            throw new InvalidOperationException($"Shell page '{descriptor.Id}' is registered without a view factory.");
        }

        return create(client);
    }
}
