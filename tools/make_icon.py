"""Regenerates the app icon. Usage: python tools/make_icon.py assets/backreel.ico

The mark is a snow-capped peak against a dusk sky. Each size is drawn on its own
rather than scaled down, because facets and back ridges turn to mush at tray
size.
"""
import sys

import numpy as np
from PIL import Image, ImageChops, ImageDraw

SIZES = (16, 20, 24, 28, 32, 40, 48, 64, 256)
SUPERSAMPLE = 8
NAVY = (9, 24, 58)
DEEP = (15, 56, 108)
SLATE = (24, 74, 132)
SKY = (32, 128, 186)
HORIZON = (150, 226, 240)
SNOW = (255, 255, 255)
SNOW_SHADE = (190, 218, 240)

PEAK = ((0.5, 0.15), (0.65, 0.38), (0.74, 0.46), (1.1, 1), (-0.1, 1), (0.25, 0.47), (0.35, 0.39))
SNOW_LINE = ((0, 0), (1, 0), (1, 0.47), (0.71, 0.47), (0.61, 0.41), (0.54, 0.52), (0.45, 0.42), (0.37, 0.5), (0.3, 0.44), (0, 0.47))
SHADED_FACE = ((0.5, 0.15), (0.47, 0.32), (0.55, 0.5), (0.49, 0.68), (0.6, 0.86), (0.56, 1), (1.1, 1), (1.1, 0.15))
BACK_LEFT = ((-0.1, 0.8), (0.17, 0.5), (0.5, 1), (-0.1, 1))
BACK_RIGHT = ((0.5, 1), (0.84, 0.54), (1.1, 0.8), (1.1, 1))
FOOTHILLS = ((0, 0.9), (0.24, 0.81), (0.5, 0.9), (0.78, 0.82), (1, 0.89), (1, 1), (0, 1))

SMALL_PEAK = ((0.5, 0.12), (1.12, 1), (-0.12, 1))
SMALL_SNOW_LINE = ((0, 0), (1, 0), (1, 0.5), (0.68, 0.5), (0.58, 0.42), (0.5, 0.54), (0.42, 0.42), (0.32, 0.5), (0, 0.5))


def sky(n, stops):
    t = np.linspace(0, 1, n)
    column = [np.interp(t, [stop for stop, _ in stops], [color[c] for _, color in stops]) for c in range(3)]
    return Image.fromarray(np.stack(column, axis=-1)[:, None, :].astype("uint8")).resize((n, n))


def shape(n, points):
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).polygon([(x * n, y * n) for x, y in points], fill=255)
    return mask


def render(size):
    n = size * SUPERSAMPLE
    full = size >= 40

    if full:
        art = sky(n, ((0, NAVY), (0.5, SKY), (0.8, HORIZON)))
        art.paste(SLATE, (0, 0), shape(n, BACK_LEFT))
        art.paste(SLATE, (0, 0), shape(n, BACK_RIGHT))
        peak = shape(n, PEAK)
        snow = ImageChops.multiply(peak, shape(n, SNOW_LINE))
        shaded = shape(n, SHADED_FACE)
        art.paste(DEEP, (0, 0), peak)
        art.paste(NAVY, (0, 0), ImageChops.multiply(peak, shaded))
        art.paste(SNOW, (0, 0), snow)
        art.paste(SNOW_SHADE, (0, 0), ImageChops.multiply(snow, shaded))
        art.paste(NAVY, (0, 0), shape(n, FOOTHILLS))
    else:
        art = sky(n, ((0, DEEP), (0.8, SKY)))
        peak = shape(n, SMALL_PEAK)
        art.paste(NAVY, (0, 0), peak)
        art.paste(SNOW, (0, 0), ImageChops.multiply(peak, shape(n, SMALL_SNOW_LINE)))

    tile = Image.new("L", (n, n), 0)
    edge = n * (0.03 if full else 0)
    ImageDraw.Draw(tile).rounded_rectangle((edge, edge, n - edge, n - edge), n * 0.22, fill=255)
    icon = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    icon.paste(art, (0, 0), tile)
    return icon.resize((size, size), Image.LANCZOS)


if __name__ == "__main__":
    frames = [render(size) for size in SIZES]
    frames[-1].save(sys.argv[1], sizes=[(size, size) for size in SIZES], append_images=frames[:-1])
