#!/usr/bin/env python3
"""Renders the Strata app-icon and splash source art from design/brand.

The mark is read from design/brand/strata-symbol.svg (the single source of the
geometry); colours are the SCREEN_SPEC tokens: abyss #0F1B26 (tile, dark
background), tide #2477B3 (mark), mist #F1F5F7 (light background).

Writes (committed) into apps/strata/assets/brand/:
  icon-ios.png              1024 full-bleed abyss square, no alpha (iOS masks it)
  icon-macos.png            1024 macOS rounded rectangle (824 body, 100 margin)
  icon-legacy.png           1024 badge (rounded abyss tile) for legacy Android
  adaptive-foreground.png   1024 = 108 dp: tide mark inside the 66 dp safe zone
  adaptive-monochrome.png   1024: the same mark in white (Android 13 themed icons)
  splash-mark.png           384 = 96 dp at xxxhdpi: tide mark, transparent
  splash-android12.png      1152: tide mark inside the 768 px (2/3) icon circle
and the desktop icons:
  windows/runner/resources/app_icon.ico   16, 20, 24, 32, 40, 48, 64, 128, 256
  linux/runner/resources/app_icon.png     256 badge (installed with the bundle)

Then run `dart run flutter_launcher_icons` and
`dart run flutter_native_splash:create` in apps/strata (docs/RUNBOOK.md §13).
Needs rsvg-convert (librsvg2-bin) and Pillow.
"""

import io
import pathlib
import re
import subprocess
import sys

from PIL import Image

APP = pathlib.Path(__file__).resolve().parents[2]
REPO = APP.parents[3]
OUT = APP / "assets" / "brand"

ABYSS = "#0F1B26"
TIDE = "#2477B3"

symbol = (REPO / "design" / "brand" / "strata-symbol.svg").read_text()
PATH = re.search(r'<path d="([^"]+)"', symbol).group(1)
STROKE = re.search(r'stroke-width="([0-9.]+)"', symbol).group(1)
# Farthest point of the mark from the centre (32, 32) of its 64-unit box: the
# round cap at (50, 13), 5.5 units beyond the path end.
MARK_RADIUS = ((50 - 32) ** 2 + (13 - 32) ** 2) ** 0.5 + float(STROKE) / 2


def mark(scale: float, colour: str = TIDE, cx: float = 32, cy: float = 32) -> str:
    return (
        f'<path transform="translate({cx} {cy}) scale({scale}) translate(-32 -32)" '
        f'd="{PATH}" stroke="{colour}" stroke-width="{STROKE}" '
        'stroke-linecap="round" stroke-linejoin="round" fill="none"/>'
    )


def svg(body: str, box: float = 64) -> str:
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {box} {box}" '
        f'width="{box}" height="{box}">{body}</svg>'
    )


def render(source: str, size: int) -> Image.Image:
    png = subprocess.run(
        ["rsvg-convert", "-w", str(size), "-h", str(size), "-f", "png"],
        input=source.encode(),
        capture_output=True,
        check=True,
    ).stdout
    return Image.open(io.BytesIO(png)).convert("RGBA")


def save(image: Image.Image, path: pathlib.Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, optimize=True)
    print(f"wrote {path.relative_to(REPO)} {image.size[0]}x{image.size[1]} {image.mode}")


BADGE = (REPO / "design" / "brand" / "strata-badge.svg").read_text()

# iOS: full bleed (the system applies its own mask); no alpha channel.
ios = svg(f'<rect width="64" height="64" fill="{ABYSS}"/>' + mark(0.62))
# macOS (Big Sur+ grid): 824 px body with a 185.4 px corner radius on a 1024 px
# canvas, leaving the transparent margin macOS icons have.
body, offset, radius = 64 * 824 / 1024, 64 * 100 / 1024, 64 * 185.4 / 1024
macos = svg(
    f'<rect x="{offset}" y="{offset}" width="{body}" height="{body}" '
    f'rx="{radius}" fill="{ABYSS}"/>' + mark(0.62 * 824 / 1024)
)
# Adaptive icon: 108 dp canvas, 66 dp safe zone (radius 33 dp); keep the mark
# within 28 dp of the centre so no launcher mask clips it.
adaptive_scale = 28 / MARK_RADIUS
foreground = svg(mark(adaptive_scale, TIDE, 54, 54), 108)
monochrome = svg(mark(adaptive_scale, "#FFFFFF", 54, 54), 108)
# Pre-12 splash: the mark alone, 96 dp.
splash = svg(mark(1.0))
# Android 12+ splash: 1152 px canvas, the icon shows inside a 768 px circle
# (radius 384); keep the mark within 300 px of the centre.
android12 = svg(mark(300 / MARK_RADIUS, TIDE, 576, 576), 1152)


def main() -> int:
    save(render(ios, 1024).convert("RGB"), OUT / "icon-ios.png")
    save(render(macos, 1024), OUT / "icon-macos.png")
    save(render(BADGE, 1024), OUT / "icon-legacy.png")
    save(render(foreground, 1024), OUT / "adaptive-foreground.png")
    save(render(monochrome, 1024), OUT / "adaptive-monochrome.png")
    save(render(splash, 384), OUT / "splash-mark.png")
    save(render(android12, 1152), OUT / "splash-android12.png")

    ico_sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    ico = APP / "windows" / "runner" / "resources" / "app_icon.ico"
    frames = [render(BADGE, s) for s in ico_sizes]
    frames[-1].save(ico, format="ICO", sizes=[(s, s) for s in ico_sizes],
                    append_images=frames[:-1])
    print(f"wrote {ico.relative_to(REPO)} {ico_sizes}")
    save(render(BADGE, 256), APP / "linux" / "runner" / "resources" / "app_icon.png")
    return 0


if __name__ == "__main__":
    sys.exit(main())
