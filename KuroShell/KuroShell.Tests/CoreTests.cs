using KuroShell.Core;
using Xunit;

namespace KuroShell.Tests;

public class CoreTests
{
    [Fact]
    public void Backoff_doubles_and_caps()
    {
        Assert.Equal(0, RestartBackoff.NextDelayMs(0));
        Assert.Equal(1000, RestartBackoff.NextDelayMs(1));
        Assert.Equal(2000, RestartBackoff.NextDelayMs(2));
        Assert.Equal(4000, RestartBackoff.NextDelayMs(3));
        Assert.Equal(30000, RestartBackoff.NextDelayMs(12));
    }

    [Fact]
    public void Config_whitelist_and_masks()
    {
        Assert.True(ShellConfig.IsAllowed("llm_mode"));
        Assert.False(ShellConfig.IsAllowed("KURO_N8N_TOKEN"));
        Assert.Equal("masked", ShellConfig.MaskNumber("12"));
        Assert.EndsWith("01", ShellConfig.MaskNumber("910000000001"));
        Assert.DoesNotContain("910000000001", ShellConfig.MaskNumber("910000000001"));
        Assert.Equal("xxxx", ShellConfig.MaskId("abcd"));
        Assert.Equal("abxxef", ShellConfig.MaskId("abcdef"));
        Assert.Contains("HUD", ShellConfig.Pages);
    }

    [Fact]
    public void Attack_is_faster_than_release()
    {
        var rise = new LevelSmoother();
        var fall = new LevelSmoother();
        for (var i = 0; i < 4; i++)
        {
            fall.Push(1, 16);
        }

        var risen = 0.0;
        var fallen = fall.Value;
        for (var i = 0; i < 3; i++)
        {
            risen = rise.Push(1, 16);
            fallen = fall.Push(0, 16);
        }

        Assert.True(risen > 0.5);
        Assert.True(fallen > 0.4);
        Assert.True(1 - risen < fallen);
    }

    [Fact]
    public void Spring_overshoot_stays_within_four_percent()
    {
        var spring = new Spring();
        var worst = 0.0;
        for (var i = 0; i < 40; i++)
        {
            var value = spring.Step(1, 0.016, animate: true);
            worst = Math.Max(worst, Math.Abs(value - 1));
        }

        Assert.True(worst <= 0.04 + 0.0001);
    }

    [Fact]
    public void Conversation_keeps_going_until_cleared_and_caps_at_twenty()
    {
        var buffer = new ConversationBuffer();
        Assert.False(buffer.Observe("hello", "", "t0"));
        Assert.True(buffer.Observe("hello", "Hi.", "t1"));
        Assert.False(buffer.Observe("hello", "Hi.", "t1"));
        Assert.False(buffer.Observe("(silence)", "nope", "t2"));
        Assert.True(buffer.Observe("again", "Still here.", "t3"));
        Assert.Equal(2, buffer.Turns.Count);

        for (var i = 0; i < 25; i++)
        {
            Assert.True(buffer.Observe("u" + i, "a" + i, "t"));
        }

        Assert.Equal(ConversationBuffer.Cap, buffer.Turns.Count);
        Assert.Equal("u24", buffer.Turns[^1].You);
        buffer.Clear();
        Assert.Empty(buffer.Turns);
        Assert.True(buffer.Observe("fresh", "Yes.", "t"));
        Assert.Equal("fresh", buffer.Turns[0].You);
    }
}
