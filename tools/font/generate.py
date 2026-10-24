#!/usr/bin/env python3
# Nwasabi Pixels: generates `src/kernel/font/data.rs`.
#
# Copyright (c) 2026 Oignontom8283 — MIT license (see `NwasabiPixels-MIT.txt`).
"""Generates the glyphs of `draw_string`: the "Nwasabi Pixels" font.

Usage, from any folder (Python 3, nothing to install):

    python3 tools/font/generate.py

The script draws every character as a few strokes, renders them as pixels in the cells of the
device (7x14 and 10x18) and rewrites `src/kernel/font/data.rs`. See `README.md`.
"""
from dataclasses import dataclass
from pathlib import Path

OUTPUT = Path(__file__).resolve().parents[2] / "src" / "kernel" / "font" / "data.rs"


# --- Font sizes ------------------------------------------------------------------------


@dataclass(frozen=True)
class Metrics:
    """Dimensions of one font size. All heights are centers of strokes."""

    name: str
    width: int  # width of the cell
    height: int  # height of the cell
    base: int  # row of the baseline: y = 0 is the top edge of row `base`
    t: float  # stroke thickness
    L: float  # center of the left vertical stroke
    R: float  # center of the right vertical stroke
    C: float  # horizontal center
    X: float  # top of lowercase letters without ascender
    K: float  # top of capitals and digits
    A: float  # top of ascenders (b, d, f, h, k, l)
    D: float  # bottom of descenders (g, j, p, q, y)
    Mx: float  # bar of lowercase letters (e, a)
    Mk: float  # junction of the bowls of capitals and bar of E, F (B, P, R...)
    Mc: float  # bar of H
    Ma: float  # bar of A
    c: int  # cut corner of round shapes (in pixels)
    dot_len: float  # vertical extension of a dot (0: one pixel)

    B = 0.5  # bottom of strokes on the baseline

    @property
    def hw(self):
        """Half the width of the body of the letters."""
        return (self.R - self.L) / 2


SMALL = Metrics("small", 7, 14, 11, 1.0, 1.5, 5.5, 3.5, 5.5, 8.5, 9.5, -2.5, 3.5, 4.5, 4.5, 3.5, 1, 0.0)
LARGE = Metrics("large", 10, 18, 14, 1.0, 1.5, 7.5, 4.5, 7.5, 11.5, 12.5, -3.5, 4.5, 6.5, 5.5, 4.5, 2, 1.0)
SIZES = [SMALL, LARGE]


# --- Strokes --------------------------------------------------------------------------------------

# A glyph is a list of *strokes*; a stroke is a series of points joined by segments. Coordinates
# are in pixels: `x` to the right from the left edge of the cell, `y` upwards from the baseline.
# The center of a one-pixel stroke therefore falls on a half-integer (`1.5` for the second column).

def line(*points):
    """A polygonal stroke through the given points."""
    return list(points)


def dot(x, y, length=0.0):
    """A dot: one pixel, or a short vertical stroke `length` longer."""
    return [(x, y), (x, y + length)] if length else [(x, y)]


def mirror(strokes, axis_x):
    """Left-right mirror image around `x = axis_x`."""
    return [[(2 * axis_x - x, y) for x, y in s] for s in strokes]


# --- Rasterization -------------------------------------------------------------------------------

# Every stroke is drawn with a round one-pixel "pen", by supersampling; a pixel covered by half or
# more is lit.

SAMPLES = 6  # sample points per pixel and per axis
LEVEL_MAX = 15  # level of a lit pixel (pixels are either off or full)


def _segments(strokes):
    segs = []
    for s in strokes:
        if len(s) == 1:
            segs.append((s[0][0], s[0][1], s[0][0], s[0][1]))
        for (x0, y0), (x1, y1) in zip(s, s[1:]):
            segs.append((x0, y0, x1, y1))
    return segs


def _dist2(px, py, seg):
    x0, y0, x1, y1 = seg
    dx, dy = x1 - x0, y1 - y0
    length2 = dx * dx + dy * dy
    u = 0.0 if length2 == 0 else max(0.0, min(1.0, ((px - x0) * dx + (py - y0) * dy) / length2))
    qx, qy = x0 + u * dx - px, y0 + u * dy - py
    return qx * qx + qy * qy


def render(strokes, m):
    """Renders the strokes in the cell of `m`: a list of `height` rows of `width` levels (0-15)."""
    segs = _segments(strokes)
    radius = m.t / 2
    r2 = radius * radius
    rows = []
    for j in range(m.height):
        row = []
        y_top = m.base - j  # top edge of the pixel, in upward y
        for i in range(m.width):
            near = [
                s
                for s in segs
                if min(s[0], s[2]) - radius <= i + 1
                and max(s[0], s[2]) + radius >= i
                and min(s[1], s[3]) - radius <= y_top
                and max(s[1], s[3]) + radius >= y_top - 1
            ]
            if not near:
                row.append(0)
                continue
            hit = 0
            for a in range(SAMPLES):
                px = i + (a + 0.5) / SAMPLES
                for b in range(SAMPLES):
                    py = y_top - (b + 0.5) / SAMPLES
                    if any(_dist2(px, py, s) <= r2 for s in near):
                        hit += 1
            coverage = hit / (SAMPLES * SAMPLES)
            row.append(LEVEL_MAX if coverage >= 0.5 else 0)
        rows.append(row)
    return rows


# --- Basic glyphs: digits, letters, punctuation --------------------------------------------


GLYPHS = {}


def glyph(*chars):
    """Registers the function for each of the given characters (or code points)."""

    def register(fn):
        for c in chars:
            GLYPHS[ord(c) if isinstance(c, str) else c] = fn
        return fn

    return register


# --- Builders ----------------------------------------------------------------------------


def octagon(l, b, r, t, c):
    """The eight vertices of a rectangle with corners cut by `c`, clockwise from the top left:
    0 and 1 at the top, 2 and 3 on the right, 4 and 5 at the bottom, 6 and 7 on the left."""
    return [
        (l + c, t),
        (r - c, t),
        (r, t - c),
        (r, b + c),
        (r - c, b),
        (l + c, b),
        (l, b + c),
        (l, t - c),
    ]


def closed(v):
    return v + [v[0]]


def walk(v, i, j, step):
    """Vertices from `i` to `j` included, clockwise (`step = 1`) or counterclockwise (`-1`)."""
    out = [v[i]]
    while i != j:
        i = (i + step) % len(v)
        out.append(v[i])
    return out


def stem(x, y0, y1):
    return line((x, y0), (x, y1))


def bar(y, x0, x1):
    return line((x0, y), (x1, y))


def rot180(strokes, m):
    """Half-turn around the center of the box of the capitals."""
    cx, cy = (m.L + m.R) / 2, (m.K + m.B) / 2
    return [[(2 * cx - x, 2 * cy - y) for x, y in s] for s in strokes]


def art(top, x0, rows):
    """A glyph drawn pixel by pixel: `top` is the top edge of the first row, `x0` the left column;
    `#` lights a pixel."""
    out = []
    for r, text in enumerate(rows):
        for c, ch in enumerate(text):
            if ch == "#":
                out.append(dot(x0 + c + 0.5, top - r - 0.5))
    return out


# --- Digits ---------------------------------------------------------------------------------


@glyph("0")
def _(m):
    c = m.c
    mid = (m.K + m.B) / 2
    return [
        closed(octagon(m.L, m.B, m.R, m.K, c)),
        line((m.C - 1, mid - 1), (m.C + 1, mid + 1)),
    ]


@glyph("1")
def _(m):
    f = m.c
    return [line((m.C - f, m.K - f), (m.C, m.K), (m.C, m.B)), bar(m.B, m.L, m.R)]


@glyph("2")
def _(m):
    c = m.c
    yd = m.K - c - 1
    return [
        line(
            (m.L, m.K - c),
            (m.L + c, m.K),
            (m.R - c, m.K),
            (m.R, m.K - c),
            (m.R, yd),
            (m.L, yd - (m.R - m.L)),
            (m.L, m.B),
            (m.R, m.B),
        )
    ]


@glyph("3")
def _(m):
    c = m.c
    return [
        line(
            (m.L, m.K - c),
            (m.L + c, m.K),
            (m.R - c, m.K),
            (m.R, m.K - c),
            (m.R, m.Mk + 1),
            (m.R - 1, m.Mk),
            (m.C - 1, m.Mk),
            (m.R - 1, m.Mk),
            (m.R, m.Mk - 1),
            (m.R, m.B + c),
            (m.R - c, m.B),
            (m.L + c, m.B),
            (m.L, m.B + c),
        )
    ]


@glyph("4")
def _(m):
    xs = m.R - 1
    return [
        line(
            (xs, m.B),
            (xs, m.K),
            (m.L, m.K - (xs - m.L)),
            (m.L, m.Ma),
            (m.R, m.Ma),
        )
    ]


@glyph("5")
def _(m):
    c = m.c
    return [
        line(
            (m.R, m.K),
            (m.L, m.K),
            (m.L, m.Mk),
            (m.R - c, m.Mk),
            (m.R, m.Mk - c),
            (m.R, m.B + c),
            (m.R - c, m.B),
            (m.L + c, m.B),
            (m.L, m.B + c),
        )
    ]


@glyph("6")
def _(m):
    c = m.c
    return [
        closed(octagon(m.L, m.B, m.R, m.Mk, c)),
        line((m.R - c, m.K), (m.L + c, m.K), (m.L, m.K - c), (m.L, m.Mk - c)),
    ]


@glyph("7")
def _(m):
    d = int(m.hw)
    y1 = m.Mk + 1
    return [line((m.L, m.K), (m.R, m.K), (m.R, y1), (m.R - d, y1 - d), (m.R - d, m.B))]


@glyph("8")
def _(m):
    c = m.c
    return [
        closed(octagon(m.L, m.Mk, m.R, m.K, c)),
        closed(octagon(m.L, m.B, m.R, m.Mk, c)),
    ]


@glyph("9")
def _(m):
    return rot180(GLYPHS[ord("6")](m), m)


# --- Capitals -------------------------------------------------------------------------------


@glyph("A")
def _(m):
    c = m.c
    return [
        line((m.L, m.B), (m.L, m.K - c), (m.L + c, m.K), (m.R - c, m.K), (m.R, m.K - c), (m.R, m.B)),
        bar(m.Ma, m.L, m.R),
    ]


def bowl(m, x0, xr, top, bottom, c):
    """A bowl open on the left: from `(x0, top)` to `(x0, bottom)` through the right."""
    return line((x0, top), (xr - c, top), (xr, top - c), (xr, bottom + c), (xr - c, bottom), (x0, bottom))


@glyph("B")
def _(m):
    c = m.c
    narrow = 1 if m.name == "large" else 0
    return [
        stem(m.L, m.B, m.K),
        bowl(m, m.L, m.R - narrow, m.K, m.Mk, c),
        bowl(m, m.L, m.R, m.Mk, m.B, c),
    ]


@glyph("C")
def _(m):
    v = octagon(m.L, m.B, m.R, m.K, m.c)
    return [walk(v, 2, 3, -1)]


@glyph("D")
def _(m):
    return [stem(m.L, m.B, m.K), bowl(m, m.L, m.R, m.K, m.B, m.c)]


@glyph("E")
def _(m):
    return [
        line((m.R, m.K), (m.L, m.K), (m.L, m.B), (m.R, m.B)),
        bar(m.Mk, m.L, m.R - 1),
    ]


@glyph("F")
def _(m):
    return [line((m.R, m.K), (m.L, m.K), (m.L, m.B)), bar(m.Mk, m.L, m.R - 1)]


@glyph("G")
def _(m):
    v = octagon(m.L, m.B, m.R, m.K, m.c)
    return [walk(v, 2, 3, -1) + [(m.R, m.Mk), (m.C, m.Mk)]]


@glyph("H")
def _(m):
    return [stem(m.L, m.B, m.K), stem(m.R, m.B, m.K), bar(m.Mc, m.L, m.R)]


@glyph("I")
def _(m):
    w = 1 if m.name == "small" else 2
    return [stem(m.C, m.B, m.K), bar(m.K, m.C - w, m.C + w), bar(m.B, m.C - w, m.C + w)]


@glyph("J")
def _(m):
    c = m.c
    return [line((m.R, m.K), (m.R, m.B + c), (m.R - c, m.B), (m.L + c, m.B), (m.L, m.B + c))]


@glyph("K")
def _(m):
    return [
        stem(m.L, m.B, m.K),
        line((m.R, m.K), (m.L + 1, m.Mc), (m.R, m.B)),
    ]


@glyph("L")
def _(m):
    return [line((m.L, m.K), (m.L, m.B), (m.R, m.B))]


@glyph("M")
def _(m):
    d = int(m.hw)
    pts = [(m.L, m.B), (m.L, m.K)]
    for i in range(1, d + 1):
        pts.append((m.L + i, m.K - i))
    for i in range(d - 1, -1, -1):
        pts.append((m.R - i, m.K - i))
    pts.append((m.R, m.B))
    return [pts]


@glyph("N")
def _(m):
    return [line((m.L, m.B), (m.L, m.K), (m.L + 1, m.K - 1), (m.R - 1, m.B + 1), (m.R, m.B), (m.R, m.K))]


@glyph("O")
def _(m):
    return [closed(octagon(m.L, m.B, m.R, m.K, m.c))]


@glyph("P")
def _(m):
    return [stem(m.L, m.B, m.K), bowl(m, m.L, m.R, m.K, m.Mk, m.c)]


@glyph("Q")
def _(m):
    return [
        closed(octagon(m.L, m.B, m.R, m.K, m.c)),
        line((m.C, m.B + 1), (m.R + 1, m.B - 1)),
    ]


@glyph("R")
def _(m):
    return [
        stem(m.L, m.B, m.K),
        bowl(m, m.L, m.R, m.K, m.Mk, m.c),
        line((m.C, m.Mk), (m.R, m.B)),
    ]


def s_path(m, top, bottom, mid, c):
    return line(
        (m.R, top - c),
        (m.R - c, top),
        (m.L + c, top),
        (m.L, top - c),
        (m.L, mid + c),
        (m.L + c, mid),
        (m.R - c, mid),
        (m.R, mid - c),
        (m.R, bottom + c),
        (m.R - c, bottom),
        (m.L + c, bottom),
        (m.L, bottom + c),
    )


@glyph("S")
def _(m):
    return [s_path(m, m.K, m.B, m.Mk, m.c)]


@glyph("T")
def _(m):
    return [bar(m.K, m.L, m.R), stem(m.C, m.B, m.K)]


@glyph("U")
def _(m):
    c = m.c
    return [line((m.L, m.K), (m.L, m.B + c), (m.L + c, m.B), (m.R - c, m.B), (m.R, m.B + c), (m.R, m.K))]


@glyph("V")
def _(m):
    return [line((m.L, m.K), (m.L, m.Mk), (m.C, m.B), (m.R, m.Mk), (m.R, m.K))]


@glyph("W")
def _(m):
    return [
        line(
            (m.L, m.K),
            (m.L, m.B + 1),
            (m.L + 1, m.B),
            (m.C, m.Mk - 1),
            (m.R - 1, m.B),
            (m.R, m.B + 1),
            (m.R, m.K),
        )
    ]


@glyph("X")
def _(m):
    return [line((m.L, m.K), (m.R, m.B)), line((m.R, m.K), (m.L, m.B))]


@glyph("Y")
def _(m):
    return [line((m.L, m.K), (m.L, m.K - 1), (m.C, m.Mc), (m.R, m.K - 1), (m.R, m.K)), stem(m.C, m.B, m.Mc)]


@glyph("Z")
def _(m):
    yd = m.K - m.hw
    return [line((m.L, m.K), (m.R, m.K), (m.R, yd), (m.L, yd - (m.R - m.L)), (m.L, m.B), (m.R, m.B))]


# --- Lowercase letters -------------------------------------------------------------------------------


@glyph("a")
def _(m):
    c = m.c
    return [
        line((m.L, m.X - c), (m.L + c, m.X), (m.R - c, m.X), (m.R, m.X - c), (m.R, m.B)),
        line((m.R, m.Mx), (m.L + c, m.Mx), (m.L, m.Mx - c), (m.L, m.B + c), (m.L + c, m.B), (m.R, m.B)),
    ]


@glyph("b")
def _(m):
    return [stem(m.L, m.A, m.B), bowl(m, m.L, m.R, m.X, m.B, m.c)]


@glyph("c")
def _(m):
    v = octagon(m.L, m.B, m.R, m.X, m.c)
    return [walk(v, 2, 3, -1)]


@glyph("d")
def _(m):
    return mirror([stem(m.L, m.A, m.B), bowl(m, m.L, m.R, m.X, m.B, m.c)], m.C)


@glyph("e")
def _(m):
    c = m.c
    return [
        line(
            (m.L, m.Mx),
            (m.R, m.Mx),
            (m.R, m.X - c),
            (m.R - c, m.X),
            (m.L + c, m.X),
            (m.L, m.X - c),
            (m.L, m.B + c),
            (m.L + c, m.B),
            (m.R - c, m.B),
            (m.R, m.B + c),
        )
    ]


@glyph("f")
def _(m):
    c = m.c
    sf = m.C - 1 if m.name == "small" else m.C - 1
    return [
        line((sf, m.B), (sf, m.A - c), (sf + c, m.A), (m.R, m.A)),
        bar(m.X, m.L, m.R - 1),
    ]


@glyph("g")
def _(m):
    c = m.c
    return [
        closed(octagon(m.L, m.B, m.R, m.X, c)),
        line((m.R, m.X), (m.R, m.D + c), (m.R - c, m.D), (m.L + c, m.D), (m.L, m.D + c)),
    ]


@glyph("h")
def _(m):
    c = m.c
    return [
        stem(m.L, m.A, m.B),
        line((m.L, m.X - c), (m.L + c, m.X), (m.R - c, m.X), (m.R, m.X - c), (m.R, m.B)),
    ]


def dot_y(m):
    return m.X + (2.0 if m.name == "small" else 3.0)


@glyph("i")
def _(m):
    return [line((m.C - 1, m.X), (m.C, m.X), (m.C, m.B)), dot(m.C, dot_y(m), m.dot_len)]


@glyph("j")
def _(m):
    c = m.c
    x = m.C
    return [
        line((x - 1, m.X), (x, m.X), (x, m.D + c), (x - c, m.D), (m.L, m.D)),
        dot(x, dot_y(m), m.dot_len),
    ]


@glyph("k")
def _(m):
    return [stem(m.L, m.A, m.B), line((m.R, m.X), (m.L + 1, m.Mx), (m.R, m.B))]


@glyph("l")
def _(m):
    c = m.c
    return [line((m.C - 1, m.A), (m.C, m.A), (m.C, m.B + c), (m.C + c, m.B), (m.R, m.B))]


@glyph("m")
def _(m):
    pts1 = [(m.L, m.X - 1), (m.L + 1, m.X), (m.C - 1, m.X), (m.C, m.X - 1), (m.C, m.B)]
    pts2 = [(m.C, m.X - 1), (m.C + 1, m.X), (m.R - 1, m.X), (m.R, m.X - 1), (m.R, m.B)]
    return [stem(m.L, m.X, m.B), pts1, pts2]


@glyph("n")
def _(m):
    c = m.c
    return [
        stem(m.L, m.X, m.B),
        line((m.L, m.X - c), (m.L + c, m.X), (m.R - c, m.X), (m.R, m.X - c), (m.R, m.B)),
    ]


@glyph("o")
def _(m):
    return [closed(octagon(m.L, m.B, m.R, m.X, m.c))]


@glyph("p")
def _(m):
    return [stem(m.L, m.X, m.D), bowl(m, m.L, m.R, m.X, m.B, m.c)]


@glyph("q")
def _(m):
    return mirror([stem(m.L, m.X, m.D), bowl(m, m.L, m.R, m.X, m.B, m.c)], m.C)


@glyph("r")
def _(m):
    c = m.c
    return [stem(m.L, m.X, m.B), line((m.L, m.X - c), (m.L + c, m.X), (m.R, m.X))]


@glyph("s")
def _(m):
    c = 1
    return [s_path(m, m.X, m.B, m.Mx, c)]


@glyph("t")
def _(m):
    c = m.c
    sf = m.C - 1
    top = m.X + (3 if m.name == "small" else 3)
    return [
        line((sf, top), (sf, m.B + c), (sf + c, m.B), (m.R, m.B)),
        bar(m.X, m.L, m.R - 1),
    ]


@glyph("u")
def _(m):
    c = m.c
    return [
        line((m.L, m.X), (m.L, m.B + c), (m.L + c, m.B), (m.R, m.B)),
        stem(m.R, m.X, m.B),
    ]


@glyph("v")
def _(m):
    return [line((m.L, m.X), (m.L, m.Mx), (m.C, m.B), (m.R, m.Mx), (m.R, m.X))]


@glyph("w")
def _(m):
    return [
        line(
            (m.L, m.X),
            (m.L, m.B + 1),
            (m.L + 1, m.B),
            (m.C, m.Mx - 1),
            (m.R - 1, m.B),
            (m.R, m.B + 1),
            (m.R, m.X),
        )
    ]


@glyph("x")
def _(m):
    if m.name == "small":
        return art(m.X + 0.5, 1, ["#...#", ".#.#.", "..#..", "..#..", ".#.#.", "#...#"])
    return art(
        m.X + 0.5,
        1,
        ["#.....#", ".#...#.", "..#.#..", "...#...", "...#...", "..#.#..", ".#...#.", "#.....#"],
    )


@glyph("y")
def _(m):
    return [
        line((m.L, m.X), (m.L, m.Mx), (m.C, m.B)),
        line((m.R, m.X), (m.R, m.Mx), (m.C, m.B), (m.C - 1, m.B - 1), (m.L, m.D + 1), (m.L, m.D)),
    ]


@glyph("z")
def _(m):
    return [line((m.L, m.X), (m.R, m.X), (m.L, m.B), (m.R, m.B))]


# --- ASCII punctuation and symbols ------------------------------------------------------------


@glyph(" ")
def _(m):
    return []


@glyph("!")
def _(m):
    return [stem(m.C, m.K, m.B + 2 + m.c), dot(m.C, m.B, m.dot_len)]


@glyph('"')
def _(m):
    n = 1 if m.name == "small" else 2
    return [stem(m.L + 1, m.K, m.K - n), stem(m.R - 1, m.K, m.K - n)]


@glyph("#")
def _(m):
    c = m.c
    return [
        stem(m.L + c, m.K, m.B),
        stem(m.R - c, m.K, m.B),
        bar(m.Mk + 1, m.L, m.R),
        bar(m.Mk - 1, m.L, m.R),
    ]


@glyph("$")
def _(m):
    ext = 1 if m.name == "small" else 2
    return [s_path(m, m.K, m.B, m.Mk, m.c), stem(m.C, m.K, m.K + ext), stem(m.C, m.B, m.B - ext)]


@glyph("%")
def _(m):
    if m.name == "small":
        return art(
            m.K + 0.5,
            1,
            ["##..#", "##.#.", "...#.", "..#..", "..#..", ".#...", ".#...", "#..##", "...##"],
        )
    return art(
        m.K + 0.5,
        1,
        [
            "###...#",
            "#.#...#",
            "###..#.",
            ".....#.",
            "....#..",
            "....#..",
            "...#...",
            "...#...",
            "..#.###",
            "..#.#.#",
            ".#..###",
            ".#.....",
        ],
    )


@glyph("&")
def _(m):
    if m.name == "small":
        return art(
            m.K + 0.5,
            1,
            [".##..", "#..#.", "#..#.", "#.#..", ".#...", "#.#.#", "#..#.", "#..#.", ".##.#"],
        )
    return art(
        m.K + 0.5,
        1,
        [
            "..###..",
            ".#...#.",
            ".#...#.",
            ".#...#.",
            "..#.#..",
            "...#...",
            "..#.#.#",
            ".#...##",
            ".#....#",
            ".#...##",
            "..###.#",
            "....#.#",
        ][:12],
    )


@glyph("'")
def _(m):
    n = 1 if m.name == "small" else 2
    return [stem(m.C, m.K, m.K - n)]


@glyph("(")
def _(m):
    c = m.c
    return [line((m.C + 1, m.K), (m.C, m.K - c), (m.C, m.B + c - 1 + 0), (m.C + 1, m.B - 1))]


@glyph(")")
def _(m):
    return mirror(GLYPHS[ord("(")](m), m.C)


@glyph("*")
def _(m):
    top = m.K + 0.5
    if m.name == "small":
        return art(top, 2, ["#.#", ".#.", "#.#"])
    return art(top, 2, ["..#..", "#.#.#", ".###.", "#.#.#", "..#.."])


@glyph("+")
def _(m):
    r = 2 if m.name == "small" else 3
    y = m.Mc
    return [bar(y, m.C - r, m.C + r), stem(m.C, y - r, y + r)]


@glyph(",")
def _(m):
    return [line((m.C, m.B), (m.C, m.B - 1), (m.C - 1, m.B - 2))]


@glyph("-", "­")
def _(m):
    return [bar(m.Mx, m.L, m.R - 1)]


@glyph(".")
def _(m):
    return [dot(m.C, m.B, m.dot_len)]


@glyph("/")
def _(m):
    return [line((m.R, m.K), (m.L, m.B - 1))]


@glyph(":")
def _(m):
    return [dot(m.C, m.B, m.dot_len), dot(m.C, m.X - m.dot_len - 1 if m.name == "large" else m.X - 1, m.dot_len)]


@glyph(";")
def _(m):
    return [
        line((m.C, m.B), (m.C, m.B - 1), (m.C - 1, m.B - 2)),
        dot(m.C, m.X - m.dot_len - 1 if m.name == "large" else m.X - 1, m.dot_len),
    ]


@glyph("<")
def _(m):
    r = 2 if m.name == "small" else 3
    return [line((m.C + r - 1, m.Mc + r), (m.C - r + 1, m.Mc), (m.C + r - 1, m.Mc - r))]


@glyph("=")
def _(m):
    return [bar(m.Mc + 1, m.L, m.R), bar(m.Mc - 1, m.L, m.R)]


@glyph(">")
def _(m):
    return mirror(GLYPHS[ord("<")](m), m.C)


@glyph("?")
def _(m):
    c = m.c
    return [
        line((m.L, m.K - c), (m.L + c, m.K), (m.R - c, m.K), (m.R, m.K - c), (m.R, m.Mk + 1), (m.C, m.Mk), (m.C, m.B + 2 + m.c)),
        dot(m.C, m.B, m.dot_len),
    ]


@glyph("@")
def _(m):
    if m.name == "small":
        return art(m.A + 0.5, 1, [".###.", "#...#", "#.###", "#.#.#", "#.###", "#....", ".###."])
    return art(
        m.A + 0.5,
        1,
        [
            "..###..",
            ".#...#.",
            "#..##.#",
            "#.#..##",
            "#.#..##",
            "#.#..##",
            "#..###.",
            "#......",
            ".#....#",
            "..####.",
        ],
    )


@glyph("[")
def _(m):
    return [line((m.C + 1, m.K), (m.C, m.K), (m.C, m.B - 1), (m.C + 1, m.B - 1))]


@glyph("\\")
def _(m):
    return [line((m.L, m.K), (m.R, m.B - 1))]


@glyph("]")
def _(m):
    return mirror(GLYPHS[ord("[")](m), m.C)


@glyph("^")
def _(m):
    top = m.K + 0.5
    if m.name == "small":
        return art(top, 2, [".#.", "#.#"])
    return art(top, 2, ["..#..", ".#.#.", "#...#"])


@glyph("_")
def _(m):
    return [bar(m.B - 2 if m.name == "small" else m.B - 2, m.L - 1, m.R + 1)]


@glyph("`")
def _(m):
    return [line((m.C - 1, m.K), (m.C, m.K - 1))]


@glyph("{")
def _(m):
    return [
        line((m.C + 1, m.K), (m.C, m.K - 1), (m.C, m.Mc + 1), (m.C - 1, m.Mc), (m.C, m.Mc - 1), (m.C, m.B), (m.C + 1, m.B - 1))
    ]


@glyph("|")
def _(m):
    return [stem(m.C, m.K, m.B - 2)]


@glyph("}")
def _(m):
    return mirror(GLYPHS[ord("{")](m), m.C)


@glyph("~")
def _(m):
    if m.name == "small":
        return art(m.Mc + 1.5, 1, [".#...", "#.#.#", "...#."])
    return art(m.Mc + 1.5, 1, [".##..#.", "#..##.#"])


# --- Accents, special letters, symbols, Greek and mathematics -----------------------------


def mark(kind, m, caps, cx=None):
    """Strokes of an accent. `caps`: put on a capital (only two rows are available)."""
    cx = m.C if cx is None else cx
    y0 = m.K + 1 if caps else m.X + 2
    three = m.name == "large" and not caps
    if kind == "acute":
        return [line((cx - 1, y0), (cx, y0 + 1), (cx + 1, y0 + 2))] if three else [line((cx, y0), (cx + 1, y0 + 1))]
    if kind == "grave":
        return [line((cx - 1, y0 + 2), (cx, y0 + 1), (cx + 1, y0))] if three else [line((cx - 1, y0 + 1), (cx, y0))]
    if kind == "circumflex":
        if three:
            return [line((cx - 2, y0), (cx - 1, y0 + 1), (cx, y0 + 2), (cx + 1, y0 + 1), (cx + 2, y0))]
        return [line((cx - 1, y0), (cx, y0 + 1), (cx + 1, y0))]
    if kind == "diaeresis":
        y = m.K + 2 if caps else y0
        return [dot(cx - 1, y), dot(cx + 1, y)]
    if kind == "tilde":
        return [dot(cx - 2, y0), dot(cx - 1, y0 + 1), dot(cx, y0 + 1), dot(cx + 1, y0), dot(cx + 2, y0 + 1)]
    if kind == "ring":
        if caps or m.name == "small" and caps:
            return [dot(cx, y0), dot(cx + 1, y0), dot(cx, y0 + 1), dot(cx + 1, y0 + 1)]
        return [dot(cx, y0), dot(cx - 1, y0 + 1), dot(cx + 1, y0 + 1), dot(cx, y0 + 2)]
    if kind == "cedilla":
        if m.name == "small":
            return [line((m.C, m.B - 1), (m.C - 1, m.B - 2))]
        return [line((m.C, m.B - 1), (m.C, m.B - 2), (m.C - 1, m.B - 3))]
    raise ValueError(kind)


def dotless_i(m):
    return [GLYPHS[ord("i")](m)[0]]


def compose(base, kind, caps):
    def fn(m):
        strokes = dotless_i(m) if base == "ı" else GLYPHS[ord(base)](m)
        return strokes + mark(kind, m, caps)

    return fn


LATIN = {
    "À": ("A", "grave"), "Á": ("A", "acute"), "Â": ("A", "circumflex"), "Ã": ("A", "tilde"),
    "Ä": ("A", "diaeresis"), "Å": ("A", "ring"), "Ç": ("C", "cedilla"), "È": ("E", "grave"),
    "É": ("E", "acute"), "Ê": ("E", "circumflex"), "Ë": ("E", "diaeresis"), "Ì": ("I", "grave"),
    "Í": ("I", "acute"), "Î": ("I", "circumflex"), "Ï": ("I", "diaeresis"), "Ñ": ("N", "tilde"),
    "Ò": ("O", "grave"), "Ó": ("O", "acute"), "Ô": ("O", "circumflex"), "Õ": ("O", "tilde"),
    "Ö": ("O", "diaeresis"), "Ù": ("U", "grave"), "Ú": ("U", "acute"), "Û": ("U", "circumflex"),
    "Ü": ("U", "diaeresis"), "Ý": ("Y", "acute"), "Ÿ": ("Y", "diaeresis"),
}
for char, (base, kind) in LATIN.items():
    GLYPHS[ord(char)] = compose(base, kind, caps=True)
# The cedilla goes under the letter: the letter stays a capital but the mark does not go up.
GLYPHS[ord("Ç")] = lambda m: GLYPHS[ord("C")](m) + mark("cedilla", m, True)

LOWER = {
    "à": ("a", "grave"), "á": ("a", "acute"), "â": ("a", "circumflex"), "ã": ("a", "tilde"),
    "ä": ("a", "diaeresis"), "å": ("a", "ring"), "ç": ("c", "cedilla"), "è": ("e", "grave"),
    "é": ("e", "acute"), "ê": ("e", "circumflex"), "ë": ("e", "diaeresis"), "ì": ("ı", "grave"),
    "í": ("ı", "acute"), "î": ("ı", "circumflex"), "ï": ("ı", "diaeresis"), "ñ": ("n", "tilde"),
    "ò": ("o", "grave"), "ó": ("o", "acute"), "ô": ("o", "circumflex"), "õ": ("o", "tilde"),
    "ö": ("o", "diaeresis"), "ù": ("u", "grave"), "ú": ("u", "acute"), "û": ("u", "circumflex"),
    "ü": ("u", "diaeresis"), "ý": ("y", "acute"), "ÿ": ("y", "diaeresis"),
}
for char, (base, kind) in LOWER.items():
    GLYPHS[ord(char)] = compose(base, kind, caps=False)
GLYPHS[ord("ç")] = lambda m: GLYPHS[ord("c")](m) + mark("cedilla", m, False)

# --- Special letters --------------------------------------------------------------------


@glyph(" ")
def _(m):
    return []


@glyph("Æ")
def _(m):
    return [
        stem(m.C, m.B, m.K),
        line((m.C, m.K), (m.L, m.B)),
        bar(m.Ma, m.L + 1, m.C),
        bar(m.K, m.C, m.R),
        bar(m.Mc, m.C, m.R - 1),
        bar(m.B, m.C, m.R),
    ]


@glyph("æ")
def _(m):
    if m.name == "small":
        return art(m.X + 0.5, 0, [".##.##.", "...#..#", ".######", "#..#...", "#..#..#", ".#####."])
    return art(
        m.X + 0.5,
        0,
        [
            ".###.###.",
            "....#...#",
            ".########",
            "#...#....",
            "#...#....",
            "#...#...#",
            ".####.##.",
            "........."
        ][:7],
    )


@glyph("Œ")
def _(m):
    c = m.c
    return [
        line((m.C, m.K), (m.L + c, m.K), (m.L, m.K - c), (m.L, m.B + c), (m.L + c, m.B), (m.C, m.B)),
        stem(m.C, m.B, m.K),
        bar(m.K, m.C, m.R),
        bar(m.Mc, m.C, m.R - 1),
        bar(m.B, m.C, m.R),
    ]


@glyph("œ")
def _(m):
    c = m.c
    return [
        line((m.C, m.X), (m.L + c, m.X), (m.L, m.X - c), (m.L, m.B + c), (m.L + c, m.B), (m.C, m.B)),
        stem(m.C, m.X, m.B),
        line((m.C, m.Mx), (m.R, m.Mx), (m.R, m.X - c), (m.R - c, m.X), (m.C, m.X)),
        line((m.C, m.B), (m.R - c, m.B), (m.R, m.B + c)),
    ]


@glyph("ß")
def _(m):
    c = m.c
    return [
        stem(m.L, m.B, m.A - c),
        line(
            (m.L, m.A - c),
            (m.L + c, m.A),
            (m.R - c, m.A),
            (m.R, m.A - c),
            (m.R, m.Mc + 1),
            (m.R - 1, m.Mc),
            (m.C, m.Mc),
        ),
        line((m.C, m.Mc), (m.R - 1, m.Mc), (m.R, m.Mc - 1), (m.R, m.B + c), (m.R - c, m.B), (m.L + 1, m.B)),
    ]


@glyph("Ð")
def _(m):
    return GLYPHS[ord("D")](m) + [bar(m.Mc, m.L - 1, m.C)]


@glyph("ð")
def _(m):
    c = m.c
    return [
        closed(octagon(m.L, m.B, m.R, m.X, c)),
        line((m.R, m.X), (m.R, m.A - 1), (m.R - 1, m.A), (m.C - 1, m.A)),
        bar(m.A - 2 if m.name == "small" else m.A - 3, m.C, m.R + 1),
    ]


@glyph("Þ")
def _(m):
    d = m.c + 1
    return [stem(m.L, m.B, m.K), bowl(m, m.L, m.R, m.K - d, m.B + d, m.c)]


@glyph("þ")
def _(m):
    return [stem(m.L, m.A, m.D), bowl(m, m.L, m.R, m.X, m.B, m.c)]


@glyph("Ø")
def _(m):
    return GLYPHS[ord("O")](m) + [line((m.L, m.B), (m.R, m.K))]


@glyph("ø")
def _(m):
    return GLYPHS[ord("o")](m) + [line((m.L, m.B), (m.R, m.X))]


# --- Typography ------------------------------------------------------------------------------


@glyph("–")
def _(m):
    return [bar(m.Mx, m.L - 0.0, m.R + 1)]


@glyph("—")
def _(m):
    return [bar(m.Mx, 0.5, m.width - 0.5)]


@glyph("’")
def _(m):
    return [line((m.C + 1, m.K), (m.C + 1, m.K - 1), (m.C, m.K - 2))]


@glyph("‘")
def _(m):
    return [line((m.C, m.K - 2), (m.C, m.K - 1), (m.C + 1, m.K))]


@glyph("”")
def _(m):
    return [line((m.C, m.K), (m.C, m.K - 1), (m.C - 1, m.K - 2)), line((m.C + 2, m.K), (m.C + 2, m.K - 1), (m.C + 1, m.K - 2))]


@glyph("“")
def _(m):
    return [line((m.C - 1, m.K - 2), (m.C - 1, m.K - 1), (m.C, m.K)), line((m.C + 1, m.K - 2), (m.C + 1, m.K - 1), (m.C + 2, m.K))]


@glyph("•")
def _(m):
    if m.name == "small":
        return art(m.Mc + 1.5, 2, [".#.", "###", ".#."])
    return art(m.Mc + 2, 3, [".##.", "####", "####", ".##."])


@glyph("…")
def _(m):
    return [dot(m.L, m.B), dot(m.C, m.B), dot(m.R, m.B)]


@glyph("€")
def _(m):
    return GLYPHS[ord("C")](m) + [bar(m.Mc + 1, m.L - 1, m.C + 1), bar(m.Mc - 1, m.L - 1, m.C + 1)]


# --- Latin-1 symbols -------------------------------------------------------------------------


@glyph("¡")
def _(m):
    return [line((m.C, m.K), (m.C, m.K - m.dot_len)), stem(m.C, m.K - 2 - m.c, m.B)]


@glyph("¿")
def _(m):
    return rot180(GLYPHS[ord("?")](m), m)


@glyph("¢")
def _(m):
    return GLYPHS[ord("c")](m) + [stem(m.C, m.X + 1, m.B - 1)]


@glyph("£")
def _(m):
    c = m.c
    return [
        line((m.R, m.K - c), (m.R - c, m.K), (m.L + 1 + c, m.K), (m.L + 1, m.K - c), (m.L + 1, m.B)),
        bar(m.B, m.L, m.R),
        bar(m.Mc, m.L, m.C + 1),
    ]


@glyph("¤")
def _(m):
    if m.name == "small":
        return art(m.Mc + 2.5, 1, ["#...#", ".###.", ".#.#.", ".###.", "#...#"])
    return art(m.Mc + 3.5, 1, ["#.....#", ".#####.", ".#...#.", ".#...#.", ".#...#.", ".#####.", "#.....#"])


@glyph("¥")
def _(m):
    return GLYPHS[ord("Y")](m) + [bar(m.Mc - 1, m.L, m.R), bar(m.Mc - 2 if m.name == "small" else m.Mc - 3, m.L, m.R)]


@glyph("¦")
def _(m):
    return [stem(m.C, m.K, m.Mc + 1), stem(m.C, m.Mc - 1, m.B - 2)]


@glyph("§")
def _(m):
    return GLYPHS[ord("S")](m) + [dot(m.L, m.Mk - 1), dot(m.R, m.Mk + 1)]


@glyph("¨")
def _(m):
    return [dot(m.C - 1, m.K), dot(m.C + 1, m.K)]


@glyph("©")
def _(m):
    if m.name == "small":
        return art(m.K + 0.5, 1, [".###.", "#...#", "#.#.#", "#...#", ".###."])
    return art(
        m.K + 0.5,
        1,
        [".#####.", "#.....#", "#.###.#", "#.#...#", "#.#...#", "#.###.#", "#.....#", ".#####."],
    )


@glyph("ª")
def _(m):
    if m.name == "small":
        return art(m.K + 0.5, 2, [".##", "#.#", ".##", "###"])
    return art(m.K + 0.5, 2, [".##.", "#..#", "#..#", ".###", "####"])


@glyph("º")
def _(m):
    if m.name == "small":
        return art(m.K + 0.5, 2, [".#.", "#.#", ".#.", "###"])
    return art(m.K + 0.5, 2, [".##.", "#..#", "#..#", ".##.", "####"])


@glyph("«")
def _(m):
    if m.name == "small":
        return art(m.Mc + 1.5, 2, [".#.#", "#.#.", ".#.#"])
    return art(m.Mc + 2.5, 2, ["..#..#", ".#..#.", "#..#..", ".#..#.", "..#..#"])


@glyph("»")
def _(m):
    if m.name == "small":
        return art(m.Mc + 1.5, 2, ["#.#.", ".#.#", "#.#."])
    return art(m.Mc + 2.5, 2, ["#..#..", ".#..#.", "..#..#", ".#..#.", "#..#.."])


@glyph("¬")
def _(m):
    return [line((m.L, m.Mc), (m.R, m.Mc), (m.R, m.Mc - 1 - (m.c - 1)))]


@glyph("®")
def _(m):
    if m.name == "small":
        return art(m.K + 0.5, 1, [".###.", "#.#.#", "#.##.", "#.#.#", ".###."])
    return art(
        m.K + 0.5,
        1,
        [".#####.", "#.....#", "#.###.#", "#.#.#.#", "#.##..#", "#.#.#.#", "#.....#", ".#####."],
    )


@glyph("¯")
def _(m):
    return [bar(m.K + 1, m.L, m.R)]


@glyph("°")
def _(m):
    if m.name == "small":
        return art(m.K + 0.5, 2, [".#.", "#.#", ".#."])
    return art(m.K + 0.5, 3, [".##.", "#..#", "#..#", ".##."])


@glyph("±")
def _(m):
    r = 2 if m.name == "small" else 3
    y = m.Mc + 1
    return [bar(y, m.C - r + 1, m.C + r - 1), stem(m.C, y - r + 1, y + r - 1), bar(m.B + 1, m.C - r + 1, m.C + r - 1)]


@glyph("´")
def _(m):
    return [line((m.C, m.K), (m.C + 1, m.K + 1))]


@glyph("µ")
def _(m):
    c = m.c
    return [
        line((m.L, m.D), (m.L, m.B + c), (m.L + c, m.B), (m.R, m.B)),
        stem(m.L, m.X, m.B + c),
        stem(m.R, m.X, m.B),
    ]


@glyph("¶")
def _(m):
    if m.name == "small":
        return art(m.K + 0.5, 1, [".####", "###.#", "###.#", "###.#", ".##.#", "..#.#", "..#.#", "..#.#", "..#.#"])
    return art(
        m.K + 0.5,
        1,
        [
            ".######",
            "####.#.",
            "####.#.",
            "####.#.",
            "####.#.",
            ".###.#.",
            "..##.#.",
            "..##.#.",
            "..##.#.",
            "..##.#.",
            "..##.#.",
            "..##.#.",
        ],
    )


@glyph("·")
def _(m):
    return [dot(m.C, m.Mc, m.dot_len)]


@glyph("¸")
def _(m):
    return mark("cedilla", m, False)


# Superscript digits and fractions: small digits drawn separately.
SMALL_DIGITS = {
    "small": {
        "1": [".#.", "##.", ".#.", "###"],
        "2": ["##.", "..#", ".#.", "###"],
        "3": ["##.", ".##", "..#", "##."],
        "4": ["#.#", "###", "..#", "..#"],
    },
    "large": {
        "1": [".#..", "##..", ".#..", ".#..", ".#..", "###."],
        "2": [".##.", "#..#", "...#", "..#.", ".#..", "####"],
        "3": [".##.", "#..#", "..#.", "..#.", "#..#", ".##."],
        "4": ["..#.", ".##.", "#.#.", "####", "..#.", "..#."],
    },
}


def tiny(m, digit, x0, top):
    return art(top, x0, SMALL_DIGITS[m.name][digit])


def superscript(digit):
    def fn(m):
        return tiny(m, digit, 2 if m.name == "small" else 3, m.K + 0.5)

    return fn


for char, digit in (("¹", "1"), ("²", "2"), ("³", "3")):
    GLYPHS[ord(char)] = superscript(digit)


def fraction(top_digit, bottom_digit):
    def fn(m):
        if m.name == "small":
            return (
                tiny(m, top_digit, 0, m.K + 0.5)
                + tiny(m, bottom_digit, 4, m.B + 4)
                + [line((m.R - 0.0, m.K), (m.L + 1, m.B))]
            )
        return (
            tiny(m, top_digit, 0, m.K + 0.5)
            + tiny(m, bottom_digit, 6, m.B + 6)
            + [line((m.R + 1, m.K), (m.L + 1, m.B))]
        )

    return fn


for char, (a, b) in (("¼", ("1", "4")), ("½", ("1", "2")), ("¾", ("3", "4"))):
    GLYPHS[ord(char)] = fraction(a, b)


@glyph("×")
def _(m):
    r = 1 if m.name == "small" else 2
    y = m.Mc
    return [line((m.C - r, y - r), (m.C + r, y + r)), line((m.C - r, y + r), (m.C + r, y - r))]


@glyph("÷")
def _(m):
    r = 2 if m.name == "small" else 3
    return [bar(m.Mc, m.C - r, m.C + r), dot(m.C, m.Mc + 2), dot(m.C, m.Mc - 2)]


# --- Greek -------------------------------------------------------------------------------------

for greek, latin in "ΑA ΒB ΕE ΖZ ΗH ΙI ΚK ΜM ΝN ΟO ΡP ΤT ΥY ΧX".split():
    GLYPHS[ord(greek)] = GLYPHS[ord(latin)]
for greek, latin in "οo".split():
    GLYPHS[ord(greek)] = GLYPHS[ord(latin)]


@glyph("Γ")
def _(m):
    return [line((m.L, m.B), (m.L, m.K), (m.R, m.K))]


@glyph("Δ")
def _(m):
    return rot180(GLYPHS[ord("V")](m), m) + [bar(m.B, m.L, m.R)]


@glyph("Θ")
def _(m):
    return [closed(octagon(m.L, m.B, m.R, m.K, m.c)), bar(m.Mc, m.L + 1, m.R - 1)]


@glyph("Λ")
def _(m):
    return rot180(GLYPHS[ord("V")](m), m)


@glyph("Ξ")
def _(m):
    return [bar(m.K, m.L, m.R), bar(m.Mc, m.L + 1, m.R - 1), bar(m.B, m.L, m.R)]


@glyph("Π")
def _(m):
    return [line((m.L, m.B), (m.L, m.K), (m.R, m.K), (m.R, m.B))]


@glyph("Σ", "∑")
def _(m):
    return [line((m.R, m.K), (m.L, m.K), (m.C, m.Mc), (m.L, m.B), (m.R, m.B))]


@glyph("Φ")
def _(m):
    e = m.c
    return [closed(octagon(m.L, m.B + e, m.R, m.K - e, 1)), stem(m.C, m.B, m.K)]


@glyph("Ψ")
def _(m):
    ym = m.Mc - m.c
    return [
        line((m.L, m.K), (m.L, ym + m.c), (m.L + m.c, ym), (m.R - m.c, ym), (m.R, ym + m.c), (m.R, m.K)),
        stem(m.C, m.B, m.K),
    ]


@glyph("Ω")
def _(m):
    c = m.c
    return [
        line(
            (m.L, m.B),
            (m.L + 1, m.B),
            (m.L + 1, m.B + 1),
            (m.L, m.B + 2),
            (m.L, m.K - c),
            (m.L + c, m.K),
            (m.R - c, m.K),
            (m.R, m.K - c),
            (m.R, m.B + 2),
            (m.R - 1, m.B + 1),
            (m.R - 1, m.B),
            (m.R, m.B),
        )
    ]


@glyph("α")
def _(m):
    return [closed(octagon(m.L, m.B, m.R, m.X, m.c)), line((m.R, m.X), (m.R, m.B + 1), (m.R + 1, m.B))]


@glyph("β")
def _(m):
    c = m.c
    yj = m.X
    return [
        stem(m.L, m.D, m.A - c),
        line((m.L, m.A - c), (m.L + c, m.A), (m.R - c, m.A), (m.R, m.A - c), (m.R, yj + 1), (m.R - 1, yj), (m.C, yj)),
        line((m.C, yj), (m.R - 1, yj), (m.R, yj - 1), (m.R, m.B + c), (m.R - c, m.B), (m.L + c, m.B), (m.L, m.B + c), (m.L, yj)),
    ]


@glyph("γ")
def _(m):
    return [
        line((m.L, m.X), (m.L, m.Mx), (m.C, m.Mx - 1)),
        line((m.R, m.X), (m.R, m.Mx), (m.C, m.Mx - 1), (m.C, m.D)),
    ]


@glyph("δ")
def _(m):
    c = m.c
    return [
        closed(octagon(m.L, m.B, m.R, m.X, c)),
        line((m.C, m.X), (m.C, m.A - 1), (m.C + 1, m.A), (m.R, m.A)),
    ]


@glyph("ε")
def _(m):
    v = octagon(m.L, m.B, m.R, m.X, 1)
    return [walk(v, 2, 3, -1), bar(m.Mx, m.L + 1, m.C)]


@glyph("ζ")
def _(m):
    c = m.c
    return [
        line((m.R, m.A), (m.L + c, m.A), (m.L, m.A - c), (m.L, m.B + c), (m.L + c, m.B), (m.C, m.B), (m.C, m.D + 1), (m.C - 1, m.D))
    ]


@glyph("η")
def _(m):
    c = m.c
    return [
        stem(m.L, m.X, m.B),
        line((m.L, m.X - c), (m.L + c, m.X), (m.R - c, m.X), (m.R, m.X - c), (m.R, m.D)),
    ]


@glyph("θ")
def _(m):
    top = m.A
    mid = (top + m.B) / 2
    mid = mid if (mid * 2) % 2 == 1 else mid + 0.5
    return [closed(octagon(m.L, m.B, m.R, top, m.c)), bar(mid, m.L + 1, m.R - 1)]


@glyph("ι")
def _(m):
    return [line((m.C, m.X), (m.C, m.B + 1), (m.C + 1, m.B), (m.R, m.B))]


@glyph("κ")
def _(m):
    return [stem(m.L, m.X, m.B), line((m.R, m.X), (m.L + 1, m.Mx), (m.R, m.B))]


@glyph("λ")
def _(m):
    return [line((m.L, m.A), (m.R, m.B)), line((m.C - 0.5, m.Mx + 1), (m.L, m.B))]


@glyph("μ")
def _(m):
    return GLYPHS[ord("µ")](m)


@glyph("ν")
def _(m):
    return GLYPHS[ord("v")](m)


@glyph("ξ")
def _(m):
    return GLYPHS[ord("ζ")](m) + [bar(m.Mx, m.L + 1, m.C)]


@glyph("π")
def _(m):
    return [bar(m.X, m.L, m.R), stem(m.L + 1, m.X, m.B), line((m.R - 1, m.X), (m.R - 1, m.B + 1), (m.R, m.B))]


@glyph("ρ")
def _(m):
    return GLYPHS[ord("p")](m)


@glyph("σ")
def _(m):
    return [closed(octagon(m.L, m.B, m.R, m.X, m.c)), bar(m.X, m.C, m.R + 1)]


@glyph("ς")
def _(m):
    c = m.c
    return [line((m.R, m.X), (m.L + c, m.X), (m.L, m.X - c), (m.L, m.B + c), (m.L + c, m.B), (m.R, m.B))]


@glyph("τ")
def _(m):
    return [bar(m.X, m.L, m.R), line((m.C, m.X), (m.C, m.B + 1), (m.C + 1, m.B), (m.R, m.B))]


@glyph("υ")
def _(m):
    c = m.c
    return [line((m.L, m.X), (m.L, m.B + c), (m.L + c, m.B), (m.R - c, m.B), (m.R, m.B + c), (m.R, m.X))]


@glyph("φ")
def _(m):
    return [closed(octagon(m.L, m.B, m.R, m.X, m.c)), stem(m.C, m.A, m.D)]


@glyph("χ")
def _(m):
    return [line((m.L, m.X), (m.R, m.D)), line((m.R, m.X), (m.L, m.D))]


@glyph("ψ")
def _(m):
    c = m.c
    return [
        line((m.L, m.X), (m.L, m.B + c), (m.L + c, m.B), (m.R - c, m.B), (m.R, m.B + c), (m.R, m.X)),
        stem(m.C, m.A, m.D),
    ]


@glyph("ω")
def _(m):
    return [
        line((m.L, m.X), (m.L, m.B + 1), (m.L + 1, m.B), (m.C - 1, m.B), (m.C, m.B + 1), (m.C, m.X - 1)),
        line((m.C, m.B + 1), (m.C + 1, m.B), (m.R - 1, m.B), (m.R, m.B + 1), (m.R, m.X)),
    ]


# --- Mathematics and arrows -----------------------------------------------------------------


@glyph("≤")
def _(m):
    r = 2 if m.name == "small" else 3
    y = m.Mc + 1
    return [line((m.C + r - 1, y + r - 1), (m.C - r + 1, y), (m.C + r - 1, y - r + 1)), bar(m.B + 1, m.C - r + 1, m.C + r - 1)]


@glyph("≥")
def _(m):
    return mirror(GLYPHS[ord("≤")](m), m.C)


@glyph("≠")
def _(m):
    return GLYPHS[ord("=")](m) + [line((m.C + 2, m.Mc + 3), (m.C - 2, m.Mc - 3))]


@glyph("≈")
def _(m):
    if m.name == "small":
        return art(m.Mc + 2.5, 1, [".##.#", "#..#.", ".....", ".##.#", "#..#."])
    return art(m.Mc + 3.5, 1, [".##..#.", "#..##.#", ".......", ".##..#.", "#..##.#"])


@glyph("∞")
def _(m):
    if m.name == "small":
        return art(m.Mc + 1.5, 0, [".##.##.", "#..#..#", ".##.##."])
    return art(m.Mc + 2.5, 0, [".##...##.", "#..#.#..#", "#...#...#", "#..#.#..#", ".##...##."])


@glyph("√")
def _(m):
    return [line((m.R, m.K), (m.C, m.K), (m.C, m.B + 1)), line((m.L, m.Mc - 1), (m.C, m.B))]


@glyph("←")
def _(m):
    r = 2 if m.name == "small" else 3
    return [bar(m.Mc, m.L - 1, m.R + 1), line((m.L - 1 + r, m.Mc + r), (m.L - 1, m.Mc), (m.L - 1 + r, m.Mc - r))]


@glyph("→")
def _(m):
    return mirror(GLYPHS[ord("←")](m), m.C)


@glyph("↑")
def _(m):
    r = 2 if m.name == "small" else 3
    return [stem(m.C, m.B, m.K), line((m.C - r, m.K - r), (m.C, m.K), (m.C + r, m.K - r))]


@glyph("↓")
def _(m):
    r = 2 if m.name == "small" else 3
    return [stem(m.C, m.B, m.K), line((m.C - r, m.B + r), (m.C, m.B), (m.C + r, m.B + r))]


# --- Writing data.rs -------------------------------------------------------------------------


HEX = ".123456789abcdef"

# Code points the font must cover: ASCII, Latin-1, a few characters of extended Latin and of
# typography, Greek and common mathematical symbols.
WANTED = sorted(
    set(range(0x20, 0x7F))
    | set(range(0xA0, 0x100))
    | {0x152, 0x153, 0x178, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2026, 0x20AC}
    | (set(range(0x391, 0x3AA)) - {0x3A2})
    | set(range(0x3B1, 0x3CA))
    | {0x2190, 0x2191, 0x2192, 0x2193, 0x2211, 0x221A, 0x221E, 0x2248, 0x2260, 0x2264, 0x2265}
)


def render_all():
    """For every defined code point, the rendered rows of each size."""
    return {
        cp: {m.name: render(fn(m), m) for m in SIZES}
        for cp, fn in sorted(GLYPHS.items())
    }


def image(rows):
    """An image: the rows as text (`.` off, `1`-`f` levels), separated by `/`."""
    return "/".join("".join(HEX[v] for v in row) for row in rows)


def write_data(rendered, path):
    missing = [cp for cp in WANTED if cp not in rendered]
    if missing:
        raise SystemExit("missing glyphs: " + " ".join(f"U+{cp:04X}" for cp in missing))
    codes = [cp for cp in sorted(rendered) if cp in WANTED]
    lines = [
        "// \"Nwasabi Pixels\" font (c) 2026 Oignontom8283, MIT license: see tools/font/NwasabiPixels-MIT.txt.",
        "// This file is produced by tools/font/generate.py; it is part of the font, just like the script.",
        "// To change a glyph, edit the script and run it again.",
        "//",
        "// Each glyph is one image per size: rows are separated by `/`; `.` is an unlit pixel, `1` to `9`",
        "// and `a` to `f` are the gray levels 1 to 15.",
        "",
        "pub const SMALL_WIDTH: u32 = %d;" % SMALL.width,
        "pub const SMALL_HEIGHT: u32 = %d;" % SMALL.height,
        "pub const LARGE_WIDTH: u32 = %d;" % LARGE.width,
        "pub const LARGE_HEIGHT: u32 = %d;" % LARGE.height,
        "",
        "/// A character and its two images.",
        "pub struct Glyph {",
        "    pub code_point: u32,",
        "    pub small: &'static str,",
        "    pub large: &'static str,",
        "}",
        "",
        "/// The glyphs, sorted by code point.",
        "pub static GLYPHS: [Glyph; %d] = [" % len(codes),
    ]
    for cp in codes:
        char = chr(cp)
        label = char if char.isprintable() and not char.isspace() else "(space)"
        lines.append("    // %s" % label)
        lines.append(
            '    Glyph { code_point: 0x%04x, small: "%s", large: "%s" },'
            % (cp, image(rendered[cp]["small"]), image(rendered[cp]["large"]))
        )
    lines.append("];")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


def main():
    rendered = render_all()
    write_data(rendered, OUTPUT)
    print(f"{len(rendered)} glyphs written to {OUTPUT}")


if __name__ == "__main__":
    main()
