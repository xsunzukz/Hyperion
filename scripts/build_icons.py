"""Rebuild the app icons from the generated logo.

Run after scripts/build_logo.py: takes `public/hyperion logo.png` (transparent
white mark) and writes the Windows/macOS/Tauri icon set into src-tauri/icons.
The mark sits on the same near-black rounded tile the sidebar uses, so the icon
reads correctly on light and dark taskbars alike.
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
LOGO = ROOT / "public" / "hyperion logo.png"
ICONS = ROOT / "src-tauri" / "icons"

TILE = (11, 11, 14, 255)  # #0b0b0e
RING = (255, 102, 0, 90)  # accent orange, soft


def render(size: int) -> Image.Image:
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    radius = int(size * 0.22)

    draw = ImageDraw.Draw(canvas)
    draw.rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=TILE)
    inset = max(1, size // 32)
    draw.rounded_rectangle(
        (inset, inset, size - 1 - inset, size - 1 - inset),
        radius=max(1, radius - inset),
        outline=RING,
        width=max(1, size // 96),
    )

    logo = Image.open(LOGO).convert("RGBA")
    target_w = int(size * 0.72)
    target_h = max(1, round(logo.height * target_w / logo.width))
    logo = logo.resize((target_w, target_h), Image.LANCZOS)

    canvas.alpha_composite(logo, ((size - logo.width) // 2, (size - logo.height) // 2))
    return canvas


def main() -> None:
    ICONS.mkdir(parents=True, exist_ok=True)

    for name, size in (("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256)):
        render(size).save(ICONS / name, "PNG", optimize=True)
        print(f"wrote {name} ({size}px)")

    big = render(256)
    big.save(ICONS / "icon.ico", sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print("wrote icon.ico (16/32/48/64/128/256)")

    try:
        big.save(ICONS / "icon.icns")
        print("wrote icon.icns")
    except Exception as exc:  # macOS bundle only; Windows builds never read it
        print(f"skipped icon.icns: {exc}")


if __name__ == "__main__":
    main()
