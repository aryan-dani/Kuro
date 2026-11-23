namespace KuroShell;

public readonly struct SayResult
{
    public SayResult(bool ok, int status, bool matched, string reply)
    {
        Ok = ok;
        Status = status;
        Matched = matched;
        Reply = reply;
    }

    public bool Ok { get; }
    public int Status { get; }
    public bool Matched { get; }
    public string Reply { get; }
}
