"""Regenerate the app logo from the brand sheet in public/app logos.png.

The sheet holds the icon at several sizes on a flat background; this crops the
largest tile, keys that background out (keeping the tile's own rounded corners),
and writes the 512 px asset the UI uses. Run scripts/build_icons.py afterwards
to refresh the Tauri icons.
"""
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SHEET = ROOT / "public" / "app logos.png"
TARGET = ROOT / "public" / "hyperion logo.png"
SIZE = 512

SHEET_BG = (20, 23, 29)
TOLERANCE = 8


def find_largest_tile(image: Image.Image) -> tuple[int, int, int, int]:
    """Bounding box of the biggest icon tile (tiles are near-black, sheet is not)."""
    mask = image.convert("L").point(lambda v: 255 if v < 20 else 0)
    width, height = mask.size
    pixels = mask.load()

    column_has_tile = [any(pixels[x, y] for y in range(0, height, 4)) for x in range(width)]
    runs: list[tuple[int, int]] = []
    start = None
    for x, filled in enumerate(column_has_tile):
        if filled and start is None:
            start = x
        elif not filled and start is not None:
            runs.append((start, x))
            start = None
    if start is not None:
        runs.append((start, width))
    if not runs:
        return mask.getbbox()

    left, right = max(runs, key=lambda r: r[1] - r[0])
    rows = mask.crop((left, 0, right, height)).getbbox()
    top, bottom = (rows[1], rows[3]) if rows else (0, height)
    return left, top, right, bottom


def key_background(tile: Image.Image) -> Image.Image:
    """Make the sheet's flat background transparent, keeping the tile's corners."""
    tile = tile.convert("RGBA")
    alpha = Image.new("L", tile.size, 255)
    src = tile.load()
    dst = alpha.load()
    br, bg, bb = SHEET_BG
    for y in range(tile.height):
        for x in range(tile.width):
            r, g, b, _ = src[x, y]
            if abs(r - br) <= TOLERANCE and abs(g - bg) <= TOLERANCE and abs(b - bb) <= TOLERANCE:
                dst[x, y] = 0
    tile.putalpha(alpha)
    return tile


def main() -> None:
    sheet = Image.open(SHEET).convert("RGB")
    box = find_largest_tile(sheet)
    tile = key_background(sheet.crop(box))

    side = max(tile.size)
    padded = int(side * 1.04)
    canvas = Image.new("RGBA", (padded, padded), (0, 0, 0, 0))
    canvas.paste(tile, ((padded - tile.width) // 2, (padded - tile.height) // 2), tile)
    canvas = canvas.resize((SIZE, SIZE), Image.LANCZOS)
    canvas.save(TARGET, "PNG", optimize=True)

    print(f"sheet {sheet.size} -> tile {tile.size} -> {TARGET.name} {canvas.size} {TARGET.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
