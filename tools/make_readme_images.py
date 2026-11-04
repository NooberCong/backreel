"""Builds the README images from real screenshots.

Usage: python tools/make_readme_images.py <wallpaper> <dark.png> <light.png> <menu.png>

The inputs are the wallpaper shown in the demo, the settings window in each
theme, and the tray menu, all captured at 150% scale. Use a freely licensed
wallpaper: these images are published with the repository.
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

OUT = Path("assets/readme")
FONTS = Path("C:/Windows/Fonts")
WINDOW_RADIUS = 12  # Windows 11 rounds windows by 8 DIPs: 12 pixels at 150%.
SCALE = 4


def rounded_mask(size, radius):
    width, height = size
    big = Image.new("L", (width * SCALE, height * SCALE), 0)
    ImageDraw.Draw(big).rounded_rectangle((0, 0, width * SCALE - 1, height * SCALE - 1), radius * SCALE, fill=255)
    return big.resize(size, Image.LANCZOS)


def cover(image, size):
    """Scales and centre-crops `image` to fill `size`."""
    scale = max(size[0] / image.width, size[1] / image.height)
    scaled = image.resize((round(image.width * scale), round(image.height * scale)), Image.LANCZOS)
    left, top = (scaled.width - size[0]) // 2, (scaled.height - size[1]) // 2
    return scaled.crop((left, top, left + size[0], top + size[1]))


def place(canvas, window, position, radius=WINDOW_RADIUS):
    """Pastes a window with its true rounded corners and a soft shadow."""
    mask = rounded_mask(window.size, radius)
    shadow = Image.new("L", canvas.size, 0)
    shadow.paste(mask, (position[0], position[1] + 14))
    shadow = shadow.filter(ImageFilter.GaussianBlur(26)).point(lambda value: value * 0.55)
    canvas.paste((0, 0, 0), (0, 0), shadow)
    canvas.paste(window, position, mask)


def backdrop(wallpaper, size, blur=0, dim=0.0):
    image = cover(wallpaper, size)
    if blur:
        image = image.filter(ImageFilter.GaussianBlur(blur))
    if dim:
        image = Image.blend(image, Image.new("RGB", size, (6, 16, 34)), dim)
    return image


def finish(image, name, radius=20):
    rounded = Image.new("RGBA", image.size, (0, 0, 0, 0))
    rounded.paste(image, (0, 0), rounded_mask(image.size, radius))
    rounded.save(OUT / name, optimize=True)


def hero(wallpaper, window):
    size = (1600, 900)
    canvas = backdrop(wallpaper, size)

    # Darken the left side so the headline stays readable on any part of the photo.
    shade = Image.linear_gradient("L").rotate(-90).resize(size).point(lambda value: max(0, value - 96) * 1.35)
    canvas.paste((6, 16, 34), (0, 0), shade)

    logo = Image.open("assets/backreel.ico")
    logo.size = (256, 256)
    logo.load()
    logo = logo.convert("RGBA").resize((104, 104), Image.LANCZOS)
    canvas.paste(logo, (96, 262), logo)

    draw = ImageDraw.Draw(canvas)
    draw.text((92, 386), "backreel", font=ImageFont.truetype(str(FONTS / "segoeuib.ttf"), 92), fill="white")
    tagline = ImageFont.truetype(str(FONTS / "segoeui.ttf"), 38)
    draw.text((96, 512), "A new wallpaper every day.", font=tagline, fill=(226, 236, 246))
    draw.text((96, 562), "Nothing left running.", font=tagline, fill=(226, 236, 246))

    height = 800
    scaled = window.resize((round(window.width * height / window.height), height), Image.LANCZOS)
    place(canvas, scaled, (size[0] - scaled.width - 110, 50), radius=11)
    finish(canvas, "hero.png")


def themes(wallpaper, dark, light):
    gap, margin = 56, 56
    size = (dark.width + light.width + gap + 2 * margin, dark.height + 2 * margin)
    canvas = backdrop(wallpaper, size, blur=24, dim=0.25)
    place(canvas, dark, (margin, margin))
    place(canvas, light, (margin + dark.width + gap, margin))
    finish(canvas, "themes.png")


def tray(wallpaper, menu):
    size = (720, 260)
    canvas = backdrop(wallpaper, size, blur=18, dim=0.35)
    place(canvas, menu, ((size[0] - menu.width) // 2, (size[1] - menu.height) // 2), radius=11)
    finish(canvas, "tray.png", radius=16)


if __name__ == "__main__":
    wallpaper, dark, light, menu = (Image.open(path).convert("RGB") for path in sys.argv[1:5])
    OUT.mkdir(exist_ok=True)
    hero(wallpaper, dark)
    themes(wallpaper, dark, light)
    tray(wallpaper, menu)
