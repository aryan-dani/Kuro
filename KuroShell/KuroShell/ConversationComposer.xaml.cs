using KuroShell.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.System;

namespace KuroShell;

public sealed partial class ConversationComposer : UserControl
{
    public event Action<string>? Status;

    private EngineClient? _client;
    private bool _busy;

    public ConversationComposer()
    {
        InitializeComponent();
    }

    public void Bind(EngineClient client)
    {
        _client = client;
    }

    public void Fill(string text)
    {
        Input.Text = text;
        Input.Focus(FocusState.Programmatic);
        Input.Select(Input.Text.Length, 0);
    }

    private void OnKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Enter && !e.KeyStatus.IsMenuKeyDown)
        {
            e.Handled = true;
            _ = SendAsync();
        }
    }

    private void OnSend(object sender, RoutedEventArgs e) => _ = SendAsync();

    public Task SendTextAsync(string text)
    {
        Input.Text = text;
        return SendAsync();
    }

    private async Task SendAsync()
    {
        if (_busy || _client is null)
        {
            return;
        }

        var text = Input.Text.Trim();
        if (text.Length == 0)
        {
            return;
        }

        if (ShellSay.IsVoiceOnly(text))
        {
            Status?.Invoke(ShellSay.VoiceOnlyMessage);
            return;
        }

        _busy = true;
        SendButton.IsEnabled = false;
        try
        {
            var result = await _client.SayAsync(ShellSay.ExecuteBody(text));
            if (result.Status == 403)
            {
                Status?.Invoke(ShellSay.VoiceOnlyMessage);
                return;
            }

            if (!result.Ok)
            {
                Status?.Invoke("The engine API is not answering.");
                return;
            }

            var reply = result.Reply;
            if (string.IsNullOrWhiteSpace(reply))
            {
                reply = result.Matched ? "Done." : "I did not catch that.";
            }

            ConversationLog.Record(text, reply);
            Input.Text = "";
            Status?.Invoke("");
        }
        finally
        {
            _busy = false;
            SendButton.IsEnabled = true;
        }
    }
}
