"""Draw the Kuro tray mark and write the Windows icon set. Pillow only."""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "branding"
PAPER = (245, 244, 238, 255)
INK = (20, 20, 20, 255)


def draw(size: int, mode: str) -> Image.Image:
    invert = mode == "listening"
    background = INK if invert else PAPER
    foreground = PAPER if invert else INK
    image = Image.new("RGBA", (size, size), background)
    pen = ImageDraw.Draw(image)
    stroke = max(2, round(size * 0.07))
    inner_stroke = max(1, stroke // 2)
    margin = max(1, size // 32)
    pen.rectangle(
        [margin, margin, size - 1 - margin, size - 1 - margin],
        outline=foreground,
        width=stroke,
    )
    inner = margin + stroke + max(1, size // 28)
    pen.rectangle(
        [inner, inner, size - 1 - inner, size - 1 - inner],
        outline=foreground,
        width=inner_stroke,
    )
    corner = max(3, round(size * 0.22))
    pen.rectangle([0, 0, corner, corner], fill=PAPER if invert else INK)

    count = 7
    gap = max(1, size // 32)
    bar_width = max(1, size // 22)
    total = (count * bar_width) + ((count - 1) * gap)
    left = (size - total) // 2
    center = size // 2
    profile = [0.22, 0.38, 0.58, 0.72, 0.58, 0.38, 0.22]
    if mode == "processing":
        profile = [0.22, 0.38, 0.78, 0.92, 0.78, 0.38, 0.22]
    elif mode == "speaking":
        profile = [0.30, 0.52, 0.88, 1.0, 0.88, 0.52, 0.30]
    travel = int(size * 0.56)
    for index, fraction in enumerate(profile):
        height = max(2, int(travel * fraction))
        if height % 2:
            height += 1
        x = left + (index * (bar_width + gap))
        y = center - (height // 2)
        pen.rectangle([x, y, x + bar_width - 1, y + height - 1], fill=foreground)
    return image


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    modes = {
        "tray_idle.png": "idle",
        "tray_listening.png": "listening",
        "tray_processing.png": "processing",
        "tray_speaking.png": "speaking",
    }
    for name, mode in modes.items():
        draw(64, mode).save(OUT / name)
    sizes = (16, 32, 48, 256)
    images = [draw(size, "idle") for size in sizes]
    icon = OUT / "kuro.ico"
    images[-1].save(
        icon,
        format="ICO",
        sizes=[(image.width, image.height) for image in images],
        append_images=images[:-1],
    )
    shell_icon = ROOT / "KuroShell" / "KuroShell" / "Assets" / "kuro.ico"
    shell_icon.write_bytes(icon.read_bytes())


if __name__ == "__main__":
    main()
