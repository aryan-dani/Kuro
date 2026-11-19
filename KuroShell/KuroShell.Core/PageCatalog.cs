namespace KuroShell.Core;

public sealed class PageCatalog
{
    private readonly Dictionary<string, IPageDescriptor> _byId = new(StringComparer.Ordinal);

    public void Register(IPageDescriptor descriptor)
    {
        ArgumentNullException.ThrowIfNull(descriptor);
        if (string.IsNullOrWhiteSpace(descriptor.Id))
        {
            throw new ArgumentException("Page id is required.", nameof(descriptor));
        }

        _byId[descriptor.Id] = descriptor;
    }

    public IPageDescriptor Resolve(string pageId)
    {
        if (string.IsNullOrWhiteSpace(pageId) || !_byId.TryGetValue(pageId, out var descriptor))
        {
            throw new PageNotFoundException(pageId ?? "", _byId.Keys);
        }

        return descriptor;
    }

    public static PageCatalog CreateDefault()
    {
        var catalog = new PageCatalog();
        catalog.Register(new PageDescriptor(PageIds.Settings, "Settings"));
        catalog.Register(new PageDescriptor(PageIds.Health, "Health"));
        catalog.Register(new PageDescriptor(PageIds.Phrases, "Phrases"));
        return catalog;
    }
}