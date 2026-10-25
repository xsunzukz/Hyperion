"""Regenerate the app logo from public/logo_clean.png.

`logo_clean.png` is the source of truth - drop a new version in and rerun this.

The artwork is a white mark on black, so the black is keyed out: luminance
becomes the alpha channel (the glow survives), the mark is cropped, squared
with a little padding and written as a compact transparent PNG that the UI can
load at 40x40 and up.
"""
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "public" / "logo_clean.png"
TARGET = ROOT / "public" / "hyperion logo.png"
SIZE = 512


def main() -> None:
    source = Image.open(SOURCE).convert("RGBA")

    # Flatten onto black first: any transparency in the file is meant to be black,
    # then treat brightness as coverage so the background disappears cleanly.
    black = Image.new("RGBA", source.size, (0, 0, 0, 255))
    flat = Image.alpha_composite(black, source)

    alpha = flat.convert("L")
    # Ignore near-black noise so the crop hugs the actual mark
    alpha = alpha.point(lambda v: 0 if v < 8 else v)

    mark = Image.new("RGBA", source.size, (255, 255, 255, 255))
    mark.putalpha(alpha)

    bbox = mark.getchannel("A").getbbox()
    if bbox:
        mark = mark.crop(bbox)

    side = max(mark.width, mark.height)
    padded = int(side * 1.08)
    canvas = Image.new("RGBA", (padded, padded), (0, 0, 0, 0))
    canvas.paste(mark, ((padded - mark.width) // 2, (padded - mark.height) // 2), mark)
    canvas = canvas.resize((SIZE, SIZE), Image.LANCZOS)
    canvas.save(TARGET, "PNG", optimize=True)

    print(f"source : {SOURCE.name} {Image.open(SOURCE).size}")
    print(f"cropped: {mark.size} -> {TARGET.name} {canvas.size} {TARGET.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
