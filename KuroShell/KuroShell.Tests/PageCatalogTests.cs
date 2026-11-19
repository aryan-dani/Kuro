using KuroShell.Core;
using Xunit;

namespace KuroShell.Tests;

public class PageCatalogTests
{
    [Fact]
    public void Resolve_settings_returns_descriptor()
    {
        var catalog = PageCatalog.CreateDefault();
        var page = catalog.Resolve(PageIds.Settings);
        Assert.Equal(PageIds.Settings, page.Id);
        Assert.Equal("Settings", page.DisplayName);
    }

    [Fact]
    public void Resolve_unknown_fails_with_known_names()
    {
        var catalog = PageCatalog.CreateDefault();
        var ex = Assert.Throws<PageNotFoundException>(() => catalog.Resolve("Missing"));
        Assert.Equal("Missing", ex.PageId);
        Assert.Contains(PageIds.Settings, ex.KnownIds);
        Assert.Contains(PageIds.Health, ex.KnownIds);
        Assert.Contains(PageIds.Phrases, ex.KnownIds);
        Assert.Contains("Unknown shell page 'Missing'", ex.Message);
        Assert.Contains("Settings", ex.Message);
    }
}