"""Perceptual-ish screenshot comparison for the parity harness.

A pixel counts as different only when no pixel within `radius` points of it
in the other capture is within `tolerance` (max channel delta): Chromium and
Core Text anti-alias glyphs differently and may land text on different
subpixel positions, which should not fail a step, while a 1-point layout
offset, a wrong colour or a missing element still does.

Differences are grouped into regions (connected blocks). For each region the
report says whether it is a pure offset (the Corvane crop matches GHD after
shifting by dx, dy points) or a colour / content difference (median colours of
the differing pixels on both sides).
"""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont


@dataclass
class Region:
    x: float  # window points
    y: float
    w: float
    h: float
    pixels: int
    share: float  # % of the compared area
    shift: tuple[int, int] | None = None  # (dx, dy) points that best aligns Corvane to GHD
    shift_gain: float = 0.0  # residual reduction of that shift, 0..1
    ghd_color: str = ""
    corvane_color: str = ""
    element: str = ""  # GHD DOM path at the region centre
    crop: str = ""

    def hint(self) -> str:
        if self.shift and self.shift != (0, 0) and self.shift_gain > 0.6:
            dx, dy = self.shift
            parts = []
            if dx:
                parts.append(f"{abs(dx)}pt {'right' if dx > 0 else 'left'}")
            if dy:
                parts.append(f"{abs(dy)}pt {'down' if dy > 0 else 'up'}")
            return "Corvane content is offset " + " and ".join(parts) + " of GHD"
        if self.ghd_color and self.corvane_color and self.ghd_color != self.corvane_color:
            return f"colour/content: GHD {self.ghd_color} vs Corvane {self.corvane_color}"
        return "content differs"


@dataclass
class Result:
    percent: float
    size: tuple[int, int]
    regions: list[Region] = field(default_factory=list)
    size_mismatch: str = ""
    coverage: float = 0.0  # % of 4pt blocks with any difference (layout-sensitive)


def _load(path: Path) -> np.ndarray:
    return np.asarray(Image.open(path).convert("RGB"), dtype=np.int16)


def _min_shift_diff(a: np.ndarray, b: np.ndarray, r: int, candidates: np.ndarray) -> np.ndarray:
    """Per candidate pixel: min over |d| <= r of max-channel |a - shift(b, d)|.

    Only pixels that differ unshifted are candidates; everything else is 0."""
    out = np.zeros(a.shape[:2], dtype=np.int16)
    ys, xs = np.nonzero(candidates)
    if r == 0 or ys.size == 0:
        out[ys, xs] = np.abs(a[ys, xs] - b[ys, xs]).max(axis=1)
        return out
    bp = np.pad(b, ((r, r), (r, r), (0, 0)), mode="edge")
    av = a[ys, xs]
    best = None
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            d = np.abs(av - bp[ys + r + dy, xs + r + dx]).max(axis=1)
            best = d if best is None else np.minimum(best, d)
    out[ys, xs] = best
    return out


def _edges(img: np.ndarray, r: int) -> np.ndarray:
    """Pixels whose (2r+1)² neighbourhood spans more than 24 levels of luminance."""
    lum = img.mean(axis=2)
    k = max(1, r)
    p = np.pad(lum, k, mode="edge")
    h, w = lum.shape
    hi = lum.copy()
    lo = lum.copy()
    for dy in range(-k, k + 1):
        for dx in range(-k, k + 1):
            v = p[k + dy : k + dy + h, k + dx : k + dx + w]
            np.maximum(hi, v, out=hi)
            np.minimum(lo, v, out=lo)
    return (hi - lo) > 24


def _label(grid: np.ndarray) -> list[list[tuple[int, int]]]:
    try:
        from scipy import ndimage  # optional, faster

        labels, n = ndimage.label(grid)
        return [list(zip(*np.nonzero(labels == i))) for i in range(1, n + 1)]
    except ImportError:
        pass
    seen = np.zeros_like(grid, dtype=bool)
    out = []
    H, W = grid.shape
    for y, x in zip(*np.nonzero(grid)):
        if seen[y, x]:
            continue
        comp, q = [], deque([(y, x)])
        seen[y, x] = True
        while q:
            cy, cx = q.popleft()
            comp.append((cy, cx))
            for ny, nx in ((cy + 1, cx), (cy - 1, cx), (cy, cx + 1), (cy, cx - 1)):
                if 0 <= ny < H and 0 <= nx < W and grid[ny, nx] and not seen[ny, nx]:
                    seen[ny, nx] = True
                    q.append((ny, nx))
        out.append(comp)
    return out


def _hex(c) -> str:
    return "#%02x%02x%02x" % tuple(int(v) for v in c)


def _best_shift(a: np.ndarray, b: np.ndarray, max_shift: int) -> tuple[tuple[int, int], float]:
    """Shift (in the arrays' pixels) of b that best matches a; returns gain vs no shift."""
    h, w = a.shape[:2]
    m = max_shift
    if h <= 2 * m + 2 or w <= 2 * m + 2:
        return (0, 0), 0.0
    core = a[m : h - m, m : w - m].astype(np.float32)
    b = b.astype(np.float32)
    base = None
    best, best_err = (0, 0), None
    for dy in range(-m, m + 1):
        for dx in range(-m, m + 1):
            cand = b[m - dy : h - m - dy, m - dx : w - m - dx]
            err = float(np.abs(core - cand).mean())
            if dx == 0 and dy == 0:
                base = err
            if best_err is None or err < best_err - 1e-6 or (abs(err - best_err) < 1e-6 and abs(dx) + abs(dy) < abs(best[0]) + abs(best[1])):
                best, best_err = (dx, dy), err
    gain = 0.0 if not base else max(0.0, (base - best_err) / base)
    return best, gain


def compare(
    ghd_png: Path,
    corvane_png: Path,
    out_dir: Path,
    stem: str,
    scale: float,
    tolerance: int = 6,
    radius_pt: float = 1.0,
    edge_tolerance: int = 40,
    masks: list | None = None,
    region: list | None = None,
    max_regions: int = 12,
) -> Result:
    a, b = _load(ghd_png), _load(corvane_png)
    size_note = ""
    if a.shape != b.shape:
        size_note = f"GHD {a.shape[1]}x{a.shape[0]} vs Corvane {b.shape[1]}x{b.shape[0]} (cropped to the common area)"
        h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
        a, b = a[:h, :w], b[:h, :w]
    ox = oy = 0
    if region:
        x, y, w, h = (int(round(v * scale)) for v in region)
        a, b = a[y : y + h, x : x + w], b[y : y + h, x : x + w]
        ox, oy = x, y
    H, W = a.shape[:2]
    r = max(0, int(round(radius_pt * scale)))
    # flat pixels (fills, borders, hover backgrounds) must match within
    # `tolerance`; pixels next to a high-contrast edge in either capture
    # (glyph anti-aliasing) within `edge_tolerance`
    edge = _edges(a, r) | _edges(b, r)
    tol = np.where(edge, max(edge_tolerance, tolerance), tolerance).astype(np.int16)
    cand = np.abs(a - b).max(axis=2) > tol
    mism = (_min_shift_diff(a, b, r, cand) > tol) | (_min_shift_diff(b, a, r, cand) > tol)
    valid = np.ones((H, W), dtype=bool)
    for mx, my, mw, mh in masks or []:
        x0, y0 = int(mx * scale) - ox, int(my * scale) - oy
        valid[max(0, y0) : max(0, y0 + int(mh * scale)), max(0, x0) : max(0, x0 + int(mw * scale))] = False
    mism &= valid
    total = int(valid.sum()) or 1
    percent = 100.0 * int(mism.sum()) / total

    # regions on a 4pt block grid, dilated one block so nearby specks merge
    bs = max(1, int(4 * scale))
    gh, gw = (H + bs - 1) // bs, (W + bs - 1) // bs
    padded = np.zeros((gh * bs, gw * bs), dtype=np.int32)
    padded[:H, :W] = mism
    counts = padded.reshape(gh, bs, gw, bs).sum(axis=(1, 3))
    hot = counts >= max(2, bs // 2)
    vpad = np.zeros((gh * bs, gw * bs), dtype=np.int32)
    vpad[:H, :W] = valid
    valid_blocks = int((vpad.reshape(gh, bs, gw, bs).sum(axis=(1, 3)) > 0).sum()) or 1
    coverage = 100.0 * int(hot.sum()) / valid_blocks
    grown = hot.copy()
    grown[1:] |= hot[:-1]
    grown[:-1] |= hot[1:]
    grown[:, 1:] |= hot[:, :-1]
    grown[:, :-1] |= hot[:, 1:]
    regions: list[Region] = []
    for comp in _label(grown):
        ys = [c[0] for c in comp]
        xs = [c[1] for c in comp]
        y0, y1 = min(ys) * bs, min(H, (max(ys) + 1) * bs)
        x0, x1 = min(xs) * bs, min(W, (max(xs) + 1) * bs)
        px = int(mism[y0:y1, x0:x1].sum())
        if px == 0:
            continue
        regions.append(
            Region(
                x=(x0 + ox) / scale,
                y=(y0 + oy) / scale,
                w=(x1 - x0) / scale,
                h=(y1 - y0) / scale,
                pixels=px,
                share=100.0 * px / total,
            )
        )
    regions.sort(key=lambda r: r.pixels, reverse=True)
    regions = regions[:max_regions]

    out_dir.mkdir(parents=True, exist_ok=True)
    img_a = Image.fromarray(a.astype(np.uint8))
    img_b = Image.fromarray(b.astype(np.uint8))
    s = int(round(scale))
    for i, reg in enumerate(regions, 1):
        x0 = int(reg.x * scale) - ox
        y0 = int(reg.y * scale) - oy
        x1, y1 = x0 + int(reg.w * scale), y0 + int(reg.h * scale)
        sub = mism[y0:y1, x0:x1]
        if sub.any():
            reg.ghd_color = _hex(np.median(a[y0:y1, x0:x1][sub], axis=0))
            reg.corvane_color = _hex(np.median(b[y0:y1, x0:x1][sub], axis=0))
        # offset search at 1x on the region plus a margin
        m = int(10 * scale)
        cx0, cy0, cx1, cy1 = max(0, x0 - m), max(0, y0 - m), min(W, x1 + m), min(H, y1 + m)
        ca = a[cy0:cy1:s, cx0:cx1:s]
        cb = b[cy0:cy1:s, cx0:cx1:s]
        if ca.size and ca.shape[0] * ca.shape[1] < 400_000:
            (dx, dy), reg.shift_gain = _best_shift(ca, cb, 8)
            # b(p - d) matches a(p): Corvane's content sits at -d relative to GHD
            reg.shift = (-dx, -dy)
        reg.crop = _crop_triptych(img_a, img_b, mism, (cx0, cy0, cx1, cy1), out_dir / f"{stem}-r{i}.png")

    _overlay(img_a, mism, regions, scale, ox, oy, out_dir / f"{stem}-diff.png")
    return Result(percent=percent, size=(W, H), regions=regions, size_mismatch=size_note, coverage=coverage)


def _font(size: int):
    for name in ("/System/Library/Fonts/SFNSMono.ttf", "/System/Library/Fonts/Menlo.ttc"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def _overlay(img_a: Image.Image, mism: np.ndarray, regions, scale, ox, oy, path: Path):
    base = img_a.convert("L").point(lambda v: 40 + v * 0.45).convert("RGB")
    arr = np.asarray(base).copy()
    arr[mism] = (255, 40, 90)
    out = Image.fromarray(arr)
    draw = ImageDraw.Draw(out)
    font = _font(int(11 * scale))
    for i, reg in enumerate(regions, 1):
        x0, y0 = reg.x * scale - ox, reg.y * scale - oy
        x1, y1 = x0 + reg.w * scale, y0 + reg.h * scale
        draw.rectangle([x0 - 2, y0 - 2, x1 + 2, y1 + 2], outline=(255, 214, 0), width=max(2, int(scale)))
        draw.text((x0 + 2, max(0, y0 - 15 * scale)), str(i), fill=(255, 214, 0), font=font)
    out.save(path)


def _crop_triptych(img_a, img_b, mism, box, path: Path) -> str:
    x0, y0, x1, y1 = box
    ca, cb = img_a.crop(box), img_b.crop(box)
    m = Image.fromarray((mism[y0:y1, x0:x1] * 255).astype(np.uint8)).convert("RGB")
    w, h = ca.size
    zoom = max(1.0, min(4.0, 220 / max(1, h), 360 / max(1, w)))
    size = (max(1, int(w * zoom)), max(1, int(h * zoom)))
    tiles = [t.resize(size, Image.NEAREST) for t in (ca, cb, m)]
    label_h = 22
    out = Image.new("RGB", (size[0] * 3 + 16, size[1] + label_h), (30, 30, 30))
    draw = ImageDraw.Draw(out)
    font = _font(13)
    for i, (tile, label) in enumerate(zip(tiles, ("GitHub Desktop", "Corvane", "diff"))):
        x = i * (size[0] + 8)
        out.paste(tile, (x, label_h))
        draw.text((x + 4, 3), label, fill=(220, 220, 220), font=font)
    out.save(path)
    return path.name
