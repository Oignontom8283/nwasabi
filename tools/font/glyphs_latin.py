# Nwasabi Pixels : lettres accentuées, symboles, grec et mathématiques.
#
# Copyright (c) 2026 Oignontom8283 — licence MIT (voir `src/kernel/font/LICENSE`).
"""Suite de `glyphs.py` : les caractères composés à partir des glyphes de base (accents posés
au-dessus ou au-dessous), puis ceux qui sont dessinés à part."""
from geometry import dot, line, mirror
from glyphs import GLYPHS, art, bar, bowl, closed, glyph, octagon, rot180, stem, walk

# --- Accents ----------------------------------------------------------------------------------


def mark(kind, m, caps, cx=None):
    """Traits d'un accent. `caps` : posé sur une majuscule (deux rangées disponibles seulement)."""
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
# Le cédille se pose sous la lettre : la lettre reste une majuscule mais la marque ne monte pas.
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

# --- Lettres particulières --------------------------------------------------------------------


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


# --- Typographie ------------------------------------------------------------------------------


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


# --- Symboles Latin-1 -------------------------------------------------------------------------


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


# Chiffres en exposant et fractions : petits chiffres dessinés à part.
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


# --- Grec -------------------------------------------------------------------------------------

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


# --- Mathématiques et flèches -----------------------------------------------------------------


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
