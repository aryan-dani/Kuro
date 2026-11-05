# HUD design notes

Taken from five reference shots. Nothing here copies a logo, a wordmark, or a green brand color. Names of other products are not used.

## What each shot is

The first shot is a Windows toast about a cancelled dictation and holding Ctrl+Win. It is not the capsule and it is not a layout source. Kuro's hotkey stays Ctrl+Alt+Space in Python.

The second shot is a page crop, not a screen edge. A dark green pill sits above a light capsule of black bars. That green is not used. Screen-edge offset, pixel size, shadow, and the exact bar count cannot be read. The capsule is a full pill: the corner radius looks like half the height.

The third shot is the listening close-up. A dark pill, near-black fill, near-white bars, all one height in that frame. The row looks like about 11 bars. Exact count, pixel width, gap, border, and shadow cannot be measured.

The fourth shot is a settings card, not the overlay. It only informs spacing. No pixel values are readable. A selected pill is black on a light surface. Unselected pills are off-white.

The fifth shot is a tall dark pill with three dots and a spinner. That is a different pose from the horizontal bar. The HUD stays one horizontal capsule. Thinking uses a flat shimmer, not that spinner.

No shot shows the capsule against a taskbar. Position is the written spec: bottom center of the active monitor, 24 px above the work area.

## Chosen tokens

These values are choices, because the shots do not contain readable pixels.

- 11 bars
- 3 px wide
- 3 px gap
- fully rounded capsule
- 1 px hairline at low opacity
- soft shadow
- near-black fill, near-white bars
- springs about 280 ms, overshoot at most 4 percent
- result pose holds 1.5 s
- idle is hidden, with an optional 6 px dot
- error shakes horizontally and does not change color

Settings pages use the same quiet spacing as the fourth shot, without copying its labels.
