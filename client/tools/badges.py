"""Draws Exile's badges as 16x16 pixel art, in the Exile palette only.

    python client/tools/badges.py [preview.png]

Writes one PNG per badge into the client's badge font (the client shows them
next to player names; the launcher uses the same files next to the name above
your skin). With a path, it also writes an 8x enlarged preview of all of them.

Everything is centred on the 16-pixel grid: a shape spanning columns a..b is
centred when a + b == 15, and the same for rows. The badges share one size: the
tile and the cat are 14 x 14, the hexagon is 16 wide and 14 tall, and the coin
is 16 across (a circle needs a little more width to look as big as a square).
"""

import os
import struct
import sys
import zlib

OUT = os.path.join(os.path.dirname(__file__), "..", "src", "main", "resources", "assets", "exile", "textures", "font", "badge")

# The launcher's palette (crates/ui/src/theme.rs).
PALETTE = {
    ".": None,
    "k": (0x11, 0x14, 0x19),  # bg
    "d": (0x1B, 0x20, 0x29),  # surface
    "a": (0x29, 0x30, 0x3D),  # surface active
    "s": (0x34, 0x3C, 0x4A),  # border strong
    "f": (0x66, 0x71, 0x85),  # text faint
    "g": (0x97, 0xA2, 0xB3),  # text dim
    "t": (0xE6, 0xEA, 0xF0),  # text
    "m": (0x2B, 0x3E, 0x5A),  # accent dim
    "n": (0x46, 0x67, 0x95),  # selection
    "p": (0x46, 0x6C, 0xA0),  # button pressed
    "b": (0x4F, 0x79, 0xB2),  # button
    "A": (0x67, 0x8F, 0xC5),  # accent
    "L": (0x8E, 0xAE, 0xDB),  # accent light
}

N = 16


def blank():
    return [["."] * N for _ in range(N)]


def shape(inside, fill, outline, top=None, bottom=None):
    """A filled shape with a 1 px outline, a lighter first row and a darker last row."""
    img = blank()
    for y in range(N):
        for x in range(N):
            if not inside(x, y):
                continue
            edge = any(not (0 <= x + dx < N and 0 <= y + dy < N and inside(x + dx, y + dy)) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
            if edge:
                img[y][x] = outline
            elif top and not inside(x, y - 2):
                img[y][x] = top
            elif bottom and not inside(x, y + 2):
                img[y][x] = bottom
            else:
                img[y][x] = fill
    return img


def stamp_centred(img, rows, colour):
    """Draws a small grid ('#' = colour) in the exact middle; it must have even width and height."""
    h, w = len(rows), len(rows[0])
    assert w % 2 == 0 and h % 2 == 0 and all(len(r) == w for r in rows), "can't centre an odd size on 16 pixels"
    x0, y0 = (N - w) // 2, (N - h) // 2
    for dy, row in enumerate(rows):
        for dx, ch in enumerate(row):
            if ch == "#":
                img[y0 + dy][x0 + dx] = colour


def rounded_square(x, y):
    """14 x 14 (columns and rows 1..14) with rounded corners."""
    cx, cy = abs(x + 0.5 - 8), abs(y + 0.5 - 8)
    if cx > 7 or cy > 7:
        return False
    ex, ey = max(cx - 4.5, 0), max(cy - 4.5, 0)
    return ex * ex + ey * ey <= 2.6 * 2.6


def circle(x, y):
    return (x + 0.5 - 8) ** 2 + (y + 0.5 - 8) ** 2 <= 7.9 ** 2


def hexagon(x, y):
    """16 wide, 14 tall (rows 1..14), points left and right."""
    dy = abs(y + 0.5 - 8)
    return dy <= 6.5 and abs(x + 0.5 - 8) <= 8 - 3.5 * dy / 6.5


def exile():
    # The app icon's tile: dark, with the blue E.
    img = shape(rounded_square, "d", "s", top="a", bottom="k")
    stamp_centred(img, [
        "######",
        "######",
        "##....",
        "#####.",
        "#####.",
        "##....",
        "######",
        "######",
    ], "A")
    # A lighter top edge on the E.
    for x in range(5, 11):
        img[4][x] = "L"
    return img


def ogu():
    # A medallion: light blue, dark letters.
    img = shape(circle, "A", "n", top="L", bottom="b")
    stamp_centred(img, [
        ".##...###.",
        "#..#..#...",
        "#..#..#.##",
        "#..#..#..#",
        "#..#..#..#",
        ".##....##.",
    ], "k")
    return img


def early_supporter():
    # A little Exile-blue cat.
    rows = [
        "................",
        "..m..........m..",
        "..mm........mm..",
        "..mLm......mLm..",
        "..mnLmmmmmmLnm..",
        ".mAAALLLLLLAAAm.",
        ".mAAAAAAAAAAAAm.",
        ".mAkkAAAAAAkkAm.",
        ".mAktAAAAAAktAm.",
        ".mLAAAAkkAAAALm.",
        ".mAAAAmAAmAAAAm.",
        ".mAAAAAmmAAAAAm.",
        "..mAAAAAAAAAAm..",
        "...mbbbbbbbbm...",
        "....mmmmmmmm....",
        "................",
    ]
    return [list(r) for r in rows]


def dev():
    # A hexagon with code brackets.
    img = shape(hexagon, "n", "m", top="b", bottom="p")
    lt = ["..#", ".#.", "#..", "#..", ".#.", "..#"]
    sl = [".#", ".#", ".#", "#.", "#.", "#."]
    gt = ["#..", ".#.", "..#", "..#", ".#.", "#.."]
    stamp_centred(img, [a + "." + b + "." + c for a, b, c in zip(lt, sl, gt)], "t")
    return img


BADGES = {"exile": exile, "ogu": ogu, "early_supporter": early_supporter, "dev": dev}


def png(width, height, pixels):
    """RGBA rows -> PNG bytes."""
    raw = b"".join(b"\x00" + bytes(row) for row in pixels)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def rgba(img):
    rows = []
    for row in img:
        out = []
        for ch in row:
            c = PALETTE[ch]
            out += [0, 0, 0, 0] if c is None else [*c, 255]
        rows.append(out)
    return rows


def symmetric(img):
    """Left and right halves mirror each other in shape (colours may differ)."""
    return all((img[y][x] == ".") == (img[y][N - 1 - x] == ".") for y in range(N) for x in range(N))


def main():
    os.makedirs(OUT, exist_ok=True)
    images = {name: draw() for name, draw in BADGES.items()}
    for name, img in images.items():
        assert symmetric(img), f"{name} isn't centred"
        with open(os.path.join(OUT, f"{name}.png"), "wb") as f:
            f.write(png(N, N, rgba(img)))
    if len(sys.argv) > 1:
        scale, gap = 8, 16
        width = len(images) * (N * scale + gap) + gap
        height = N * scale + 2 * gap
        bg = PALETTE["k"]
        rows = [[*bg, 255] * width for _ in range(height)]
        for i, img in enumerate(images.values()):
            ox = gap + i * (N * scale + gap)
            for y in range(N * scale):
                for x in range(N * scale):
                    c = PALETTE[img[y // scale][x // scale]]
                    if c:
                        rows[gap + y][(ox + x) * 4:(ox + x) * 4 + 4] = [*c, 255]
        with open(sys.argv[1], "wb") as f:
            f.write(png(width, height, rows))


if __name__ == "__main__":
    main()
