# Nwasabi Pixels : dessin des glyphes (squelettes de traits).
#
# Copyright (c) 2026 Oignontom8283 — licence MIT (voir `src/kernel/font/LICENSE`).
"""Chaque glyphe est une fonction `m -> traits` (voir `geometry.py`), écrite avec les repères de la
taille `m` (`m.L`, `m.R`, `m.C`, `m.X`, `m.K`…) pour que le même dessin serve aux deux tailles.

Le style est celui des polices à pixels : traits droits d'un pixel, coins coupés à 45° et
diagonales d'une colonne par rangée, qui tombent exactement sur la grille. Quelques glyphes
compliqués sont dessinés pixel par pixel (`art`).
"""
from geometry import dot, line, mirror

GLYPHS = {}


def glyph(*chars):
    """Enregistre la fonction pour chacun des caractères (ou points de code) donnés."""

    def register(fn):
        for c in chars:
            GLYPHS[ord(c) if isinstance(c, str) else c] = fn
        return fn

    return register


# --- Constructeurs ----------------------------------------------------------------------------


def octagon(l, b, r, t, c):
    """Les huit sommets d'un rectangle aux coins coupés de `c`, dans le sens horaire depuis le haut
    à gauche : 0 et 1 en haut, 2 et 3 à droite, 4 et 5 en bas, 6 et 7 à gauche."""
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
    """Sommets de `i` à `j` inclus, dans le sens horaire (`step = 1`) ou inverse (`-1`)."""
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
    """Demi-tour autour du centre de la boîte des majuscules."""
    cx, cy = (m.L + m.R) / 2, (m.K + m.B) / 2
    return [[(2 * cx - x, 2 * cy - y) for x, y in s] for s in strokes]


def art(top, x0, rows):
    """Glyphe dessiné pixel par pixel : `top` est le bord haut de la première rangée, `x0` la
    colonne de gauche ; `#` allume un pixel."""
    out = []
    for r, text in enumerate(rows):
        for c, ch in enumerate(text):
            if ch == "#":
                out.append(dot(x0 + c + 0.5, top - r - 0.5))
    return out


def by_size(m, small, large):
    """Choisit le dessin de la taille de `m` parmi deux fonctions ou valeurs."""
    return small if m.name == "small" else large


# --- Chiffres ---------------------------------------------------------------------------------


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


# --- Majuscules -------------------------------------------------------------------------------


@glyph("A")
def _(m):
    c = m.c
    return [
        line((m.L, m.B), (m.L, m.K - c), (m.L + c, m.K), (m.R - c, m.K), (m.R, m.K - c), (m.R, m.B)),
        bar(m.Ma, m.L, m.R),
    ]


def bowl(m, x0, xr, top, bottom, c):
    """Panse ouverte à gauche : de `(x0, top)` à `(x0, bottom)` par la droite."""
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


# --- Minuscules -------------------------------------------------------------------------------


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


# --- Ponctuation et symboles ASCII ------------------------------------------------------------


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
