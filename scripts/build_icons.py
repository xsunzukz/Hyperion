"""Rebuild the app icons from the generated logo.

Run after scripts/build_logo.py: `public/hyperion logo.png` already contains the
finished tile (rounded corners, transparent outside), so the icon set is just
that artwork at the sizes Tauri and Windows expect.
"""
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
LOGO = ROOT / "public" / "hyperion logo.png"
ICONS = ROOT / "src-tauri" / "icons"


def render(size: int) -> Image.Image:
    logo = Image.open(LOGO).convert("RGBA")
    # Tauri wants square icons; the source already is, this is only a guard
    side = max(logo.size)
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(logo, ((side - logo.width) // 2, (side - logo.height) // 2), logo)
    return canvas.resize((size, size), Image.LANCZOS)


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
