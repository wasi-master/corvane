#!/usr/bin/env python3
"""Bake GitHub Desktop's dark-theme blankslate filter into SVG copies.

GHD draws its `empty-*.svg` illustrations (`.blankslate-image`) through

    body.theme-dark .blankslate-image {
      filter: invert() grayscale(1) brightness(8) contrast(0.6)
    }

GPUI has no CSS filters, so this rewrites every hex colour in those SVGs with
the same chain (Chromium applies each step as a colour matrix and clamps
between steps) and writes `assets/illustrations/dark/<name>.svg`, which
`widgets::blankslate_image` picks in the dark themes.

    python3 tools/illustrations/darken.py
"""

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = ROOT / "assets/illustrations"
OUT = SRC / "dark"


def clamp(v):
    return min(1.0, max(0.0, v))


def dark(r, g, b):
    r, g, b = (1 - r, 1 - g, 1 - b)  # invert()
    y = clamp(0.2126 * r + 0.7152 * g + 0.0722 * b)  # grayscale(1)
    y = clamp(y * 8)  # brightness(8)
    y = clamp((y - 0.5) * 0.6 + 0.5)  # contrast(0.6)
    return y, y, y


def rewrite(match):
    h = match.group(1)
    if len(h) == 3:
        h = "".join(c * 2 for c in h)
    rgb = [int(h[i : i + 2], 16) / 255 for i in (0, 2, 4)]
    return "#" + "".join(f"{round(c * 255):02X}" for c in dark(*rgb))


# every image GHD renders with `className="blankslate-image"`
IMAGES = [
    "empty-no-branches.svg",
    "empty-no-commit.svg",
    "empty-no-file-selected.svg",
    "empty-no-pull-requests.svg",
    "empty-no-repo.svg",
    "multiple-files-selected.svg",
    "paper-stack.svg",
    "ufo-alert.svg",
]


def main():
    OUT.mkdir(exist_ok=True)
    for svg in (SRC / name for name in IMAGES):
        text = svg.read_text()
        text = re.sub(r"#([0-9A-Fa-f]{6}|[0-9A-Fa-f]{3})\b", rewrite, text)
        (OUT / svg.name).write_text(text)
        print(OUT.relative_to(ROOT) / svg.name)


if __name__ == "__main__":
    main()
