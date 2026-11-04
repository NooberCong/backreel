"""Regenerates the app icon. Usage: python tools/make_icon.py assets/backreel.ico

The mark is an amber film reel rising like a sun behind blue ridges. Each size
is drawn on its own rather than scaled down, because five reel holes and two
ridges turn to mush at tray size.
"""
import math
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

SIZES = (16, 20, 24, 28, 32, 40, 48, 64, 256)
SUPERSAMPLE = 8
NAVY = (9, 24, 58)
DEEP = (15, 56, 108)
SKY = (32, 128, 186)
HORIZON = (150, 226, 240)
AMBER = (255, 184, 64)
BACK_RIDGE = ((0, 0.74), (0.17, 0.66), (0.36, 0.76), (0.72, 0.6), (1, 0.72), (1, 1), (0, 1))
FRONT_RIDGE = ((0, 0.88), (0.26, 0.76), (0.5, 0.87), (0.78, 0.77), (1, 0.87), (1, 1), (0, 1))
SMALL_RIDGE = ((0, 0.82), (0.3, 0.7), (0.56, 0.84), (0.8, 0.68), (1, 0.8), (1, 1), (0, 1))


def sky(n, stops):
    t = np.linspace(0, 1, n)
    column = [np.interp(t, [stop for stop, _ in stops], [color[c] for _, color in stops]) for c in range(3)]
    return Image.fromarray(np.stack(column, axis=-1)[:, None, :].astype("uint8")).resize((n, n))


def disc(draw, n, x, y, radius, fill):
    draw.ellipse(((x - radius) * n, (y - radius) * n, (x + radius) * n, (y + radius) * n), fill=fill)


def reel(n, y, radius, holes, hole, hub):
    """The reel as a mask; `hole` and `hub` are fractions of its radius."""
    mask = Image.new("L", (n, n), 0)
    draw = ImageDraw.Draw(mask)
    disc(draw, n, 0.5, y, radius, 255)
    for index in range(holes):
        angle = math.radians(-90 + index * 360 / holes)
        x, hole_y = 0.5 + radius * 0.56 * math.cos(angle), y + radius * 0.56 * math.sin(angle)
        disc(draw, n, x, hole_y, radius * hole, 0)
    if hub:
        disc(draw, n, 0.5, y, radius * hub, 0)
    return mask


def ridge(art, n, points, fill):
    ImageDraw.Draw(art).polygon([(x * n, y * n) for x, y in points], fill=fill)


def render(size):
    n = size * SUPERSAMPLE
    full = size >= 40

    if full:
        art = sky(n, ((0, NAVY), (0.6, SKY), (0.78, HORIZON)))
        mark = reel(n, 0.43, 0.29, holes=5, hole=0.235, hub=0.1)
        # A soft glow, which would only blur the smaller sizes. The clean sky goes
        # back over the reel's area so the holes don't turn muddy.
        clean = art.copy()
        solid = reel(n, 0.43, 0.29, holes=0, hole=0, hub=0)
        glow = solid.filter(ImageFilter.GaussianBlur(n * 0.05)).point(lambda value: value * 0.4)
        art.paste(AMBER, (0, 0), glow)
        art.paste(clean, (0, 0), solid)
    else:
        art = sky(n, ((0, NAVY), (0.75, SKY)))
        mark = reel(n, 0.42, 0.36, holes=3, hole=0.29, hub=0)
    art.paste(AMBER, (0, 0), mark)

    if full:
        ridge(art, n, BACK_RIDGE, DEEP)
        ridge(art, n, FRONT_RIDGE, NAVY)
    else:
        ridge(art, n, SMALL_RIDGE, NAVY)

    tile = Image.new("L", (n, n), 0)
    edge = n * (0.03 if full else 0)
    ImageDraw.Draw(tile).rounded_rectangle((edge, edge, n - edge, n - edge), n * 0.22, fill=255)
    icon = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    icon.paste(art, (0, 0), tile)
    return icon.resize((size, size), Image.LANCZOS)


if __name__ == "__main__":
    frames = [render(size) for size in SIZES]
    frames[-1].save(sys.argv[1], sizes=[(size, size) for size in SIZES], append_images=frames[:-1])
