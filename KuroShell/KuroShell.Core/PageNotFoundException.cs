namespace KuroShell.Core;

public sealed class PageNotFoundException : Exception
{
    public PageNotFoundException(string pageId, IEnumerable<string> knownIds)
        : base(FormatMessage(pageId, knownIds))
    {
        PageId = pageId;
        KnownIds = knownIds.Order(StringComparer.Ordinal).ToArray();
    }

    public string PageId { get; }

    public IReadOnlyList<string> KnownIds { get; }

    private static string FormatMessage(string pageId, IEnumerable<string> knownIds)
    {
        var label = string.IsNullOrWhiteSpace(pageId) ? "(empty)" : pageId;
        var known = string.Join(", ", knownIds.Order(StringComparer.Ordinal));
        return $"Unknown shell page '{label}'. Known pages: {known}.";
    }
}