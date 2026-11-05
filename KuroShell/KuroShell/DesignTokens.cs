using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace KuroShell;

public static class DesignTokens
{
    public const string Face = "Assets/Fonts/SpaceGrotesk-Regular.ttf#Space Grotesk";
    public const string FaceMedium = "Assets/Fonts/SpaceGrotesk-Medium.ttf#Space Grotesk";
    public const string FaceSemi = "Assets/Fonts/SpaceGrotesk-Bold.ttf#Space Grotesk";
    public const string FaceMono = "Assets/Fonts/JetBrainsMono-Regular.ttf#JetBrains Mono";

    public static int BarCount { get; set; } = 11;
    public static double BarWidth { get; set; } = 5;
    public static double BarGap { get; set; } = 6;
    public static double CapsuleHeight { get; set; } = 38;
    public static double WorkAreaGap { get; set; } = 24;
    public static int ResultHoldMs { get; set; } = 1500;

    public static double RadiusRow { get; } = 0;
    public static double RadiusCard { get; } = 0;
    public static double TitleSize { get; } = 28;
    public static double SectionSize { get; } = 15;
    public static double BodySize { get; } = 14;
    public static double MetaSize { get; } = 12;

    public static Color Canvas { get; } = Color.FromArgb(255, 245, 244, 238);
    public static Color Card { get; } = Color.FromArgb(255, 255, 255, 255);
    public static Color Rail { get; } = Color.FromArgb(255, 245, 244, 238);
    public static Color Fill { get; } = Color.FromArgb(255, 255, 255, 255);
    public static Color Ink { get; } = Color.FromArgb(255, 20, 20, 20);
    public static Color Dim { get; } = Color.FromArgb(255, 228, 226, 216);
    public static Color Muted { get; } = Color.FromArgb(255, 82, 81, 78);
    public static Color Surface { get; } = Canvas;
    public static Color Hairline { get; } = Color.FromArgb(255, 20, 20, 20);

    public static SolidColorBrush CanvasBrush { get; } = new(Canvas);
    public static SolidColorBrush CardBrush { get; } = new(Card);
    public static SolidColorBrush RailBrush { get; } = new(Rail);
    public static SolidColorBrush FillBrush { get; } = new(Fill);
    public static SolidColorBrush InkBrush { get; } = new(Ink);
    public static SolidColorBrush DimBrush { get; } = new(Dim);
    public static SolidColorBrush MutedBrush { get; } = new(Muted);
    public static SolidColorBrush HairlineBrush { get; } = new(Hairline);

    public static int CapsuleWidth
    {
        get
        {
            var bars = BarCount * BarWidth;
            var gaps = Math.Max(0, BarCount - 1) * BarGap;
            return (int)Math.Ceiling(bars + gaps + 48);
        }
    }
}
