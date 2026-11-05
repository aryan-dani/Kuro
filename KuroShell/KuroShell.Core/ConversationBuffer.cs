namespace KuroShell.Core;

public sealed class Turn
{
    public string You { get; set; } = "";
    public string Kuro { get; set; } = "";
    public string At { get; set; } = "";
}

public sealed class ConversationBuffer
{
    public const int Cap = 20;

    private readonly List<Turn> _turns = [];
    private string _committedYou = "";
    private string _committedKuro = "";
    private string _pendingYou = "";

    public IReadOnlyList<Turn> Turns => _turns;

    public void Seed(IEnumerable<Turn> turns)
    {
        _turns.Clear();
        foreach (var turn in turns)
        {
            if (string.IsNullOrWhiteSpace(turn.You) || string.IsNullOrWhiteSpace(turn.Kuro))
            {
                continue;
            }

            _turns.Add(turn);
        }

        while (_turns.Count > Cap)
        {
            _turns.RemoveAt(0);
        }

        if (_turns.Count == 0)
        {
            _committedYou = "";
            _committedKuro = "";
            _pendingYou = "";
            return;
        }

        _committedYou = _turns[^1].You;
        _committedKuro = _turns[^1].Kuro;
        _pendingYou = "";
    }

    public bool Observe(string you, string kuro, string at)
    {
        you = (you ?? "").Trim();
        kuro = (kuro ?? "").Trim();
        if (you == "(silence)")
        {
            you = "";
        }

        if (you.Length > 0 && you != _pendingYou && you != _committedYou)
        {
            _pendingYou = you;
        }

        if (_pendingYou.Length == 0 || kuro.Length == 0 || kuro == _committedKuro)
        {
            return false;
        }

        _turns.Add(new Turn { You = _pendingYou, Kuro = kuro, At = at });
        while (_turns.Count > Cap)
        {
            _turns.RemoveAt(0);
        }

        _committedYou = _pendingYou;
        _committedKuro = kuro;
        _pendingYou = "";
        return true;
    }

    public void Clear()
    {
        _turns.Clear();
        _committedYou = "";
        _committedKuro = "";
        _pendingYou = "";
    }
}
