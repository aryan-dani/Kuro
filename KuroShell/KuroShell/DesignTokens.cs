using Windows.UI;

namespace KuroShell;

public static class DesignTokens
{
    public static int BarCount { get; set; } = 11;
    public static double BarWidth { get; set; } = 3;
    public static double BarGap { get; set; } = 3;
    public static double CapsuleHeight { get; set; } = 36;
    public static double WorkAreaGap { get; set; } = 24;
    public static int ResultHoldMs { get; set; } = 1500;

    public static Color Fill { get; } = Color.FromArgb(255, 10, 10, 10);
    public static Color Ink { get; } = Color.FromArgb(255, 245, 245, 245);
    public static Color Dim { get; } = Color.FromArgb(255, 42, 42, 42);
    public static Color Muted { get; } = Color.FromArgb(255, 140, 140, 140);
    public static Color Surface { get; } = Color.FromArgb(255, 18, 18, 18);

    public static int CapsuleWidth
    {
        get
        {
            var bars = BarCount * BarWidth;
            var gaps = Math.Max(0, BarCount - 1) * BarGap;
            return (int)Math.Ceiling(bars + gaps + 28);
        }
    }
}
