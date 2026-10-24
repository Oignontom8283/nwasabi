# Nwasabi Pixels : primitives de dessin, métriques et rastérisation.
#
# Copyright (c) 2026 Oignontom8283 — licence MIT (voir `src/kernel/font/LICENSE`).
"""Primitives géométriques et rastériseur de la police « Nwasabi Pixels ».

Un glyphe est une liste de *traits* ; un trait est une suite de points reliés par des segments. Les
coordonnées sont en pixels, `x` vers la droite depuis le bord gauche de la cellule, `y` vers le
haut depuis la ligne de base. Un pixel `(i, j)` de la cellule couvre `x` de `i` à `i + 1` et `y`
de `base - j - 1` à `base - j` ; le centre d'un trait d'un pixel d'épaisseur tombe donc sur un
demi-entier (`1,5` pour la deuxième colonne).

Le rastériseur dessine chaque trait avec un « stylo » rond d'épaisseur `t`, par suréchantillonnage,
puis durcit le contraste pour que les traits droits restent nets et les courbes légèrement lissées.
"""
import math
import struct
import zlib
from dataclasses import dataclass


@dataclass(frozen=True)
class Metrics:
    """Dimensions d'une taille de police. Toutes les hauteurs sont des centres de traits."""

    name: str
    width: int  # largeur de la cellule
    height: int  # hauteur de la cellule
    base: int  # rangée de la ligne de base : y = 0 est le bord haut de la rangée `base`
    t: float  # épaisseur des traits
    L: float  # centre du trait vertical gauche
    R: float  # centre du trait vertical droit
    C: float  # centre horizontal
    X: float  # haut des minuscules sans hampe
    K: float  # haut des majuscules et des chiffres
    A: float  # haut des hampes (b, d, f, h, k, l)
    D: float  # bas des jambages (g, j, p, q, y)
    Mx: float  # barre des minuscules (e, a)
    Mk: float  # jonction des panses des majuscules et barre de E, F (B, P, R…)
    Mc: float  # barre de H
    Ma: float  # barre de A
    c: int  # coin coupé des formes rondes (en pixels)
    dot_len: float  # allongement vertical d'un point (0 : un pixel)
    exp: float = 2.6  # exposant des ovales (2 : ellipse ; plus : plus carré)

    B = 0.5  # bas des traits sur la ligne de base

    @property
    def hw(self):
        """Demi-largeur du corps des lettres."""
        return (self.R - self.L) / 2

    def y(self, value):
        """Coordonnée `y` vers le haut -> ordonnée de pixel (vers le bas)."""
        return self.base - value


SMALL = Metrics("small", 7, 14, 11, 1.0, 1.5, 5.5, 3.5, 5.5, 8.5, 9.5, -2.5, 3.5, 4.5, 4.5, 3.5, 1, 0.0)
LARGE = Metrics("large", 10, 18, 14, 1.0, 1.5, 7.5, 4.5, 7.5, 11.5, 12.5, -3.5, 4.5, 6.5, 5.5, 4.5, 2, 1.0)
SIZES = [SMALL, LARGE]


# --- Constructeurs de traits ------------------------------------------------------------------


def line(*points):
    """Trait polygonal passant par les points donnés."""
    return list(points)


def dot(x, y, length=0.0):
    """Un point : un pixel, ou un petit trait vertical de `length` de plus."""
    return [(x, y), (x, y + length)] if length else [(x, y)]


def arc(cx, cy, rx, ry, a0, a1, exp=2.0, step=5.0):
    """Arc d'ovale de centre `(cx, cy)` ; angles en degrés, 0 = droite, 90 = haut. Un `exp`
    supérieur à 2 donne un ovale plus carré (superellipse), comme dans les polices à pixels."""
    n = max(2, int(abs(a1 - a0) / step) + 1)
    pts = []
    for i in range(n + 1):
        a = math.radians(a0 + (a1 - a0) * i / n)
        c, s = math.cos(a), math.sin(a)
        pts.append(
            (
                cx + rx * math.copysign(abs(c) ** (2 / exp), c),
                cy + ry * math.copysign(abs(s) ** (2 / exp), s),
            )
        )
    return pts


def bez(p0, p1, p2, p3=None, n=10):
    """Courbe de Bézier quadratique (trois points) ou cubique (quatre)."""
    pts = []
    for i in range(n + 1):
        u = i / n
        v = 1 - u
        if p3 is None:
            pts.append(
                (
                    v * v * p0[0] + 2 * v * u * p1[0] + u * u * p2[0],
                    v * v * p0[1] + 2 * v * u * p1[1] + u * u * p2[1],
                )
            )
        else:
            pts.append(
                (
                    v**3 * p0[0] + 3 * v * v * u * p1[0] + 3 * v * u * u * p2[0] + u**3 * p3[0],
                    v**3 * p0[1] + 3 * v * v * u * p1[1] + 3 * v * u * u * p2[1] + u**3 * p3[1],
                )
            )
    return pts


def join(*parts):
    """Concatène des tracés en un seul trait continu."""
    out = []
    for part in parts:
        out.extend(part)
    return out


def transform(strokes, sx, sy, dx, dy):
    """Applique `x' = sx·x + dx`, `y' = sy·y + dy` à des traits (exposants, fractions…)."""
    return [[(sx * x + dx, sy * y + dy) for x, y in s] for s in strokes]


def mirror(strokes, axis_x):
    """Symétrie gauche-droite autour de `x = axis_x`."""
    return [[(2 * axis_x - x, y) for x, y in s] for s in strokes]


# --- Rastérisation ----------------------------------------------------------------------------

SAMPLES = 6  # points d'échantillonnage par pixel et par axe
LOW, HIGH = 0.499, 0.5  # couverture en dessous de laquelle un pixel est vide, au-dessus de laquelle il est plein
LEVELS = 15


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
    """Rend les traits dans la cellule de `m` : liste de `height` rangées de `width` niveaux (0-15)."""
    segs = _segments(strokes)
    radius = m.t / 2
    r2 = radius * radius
    rows = []
    for j in range(m.height):
        row = []
        y_top = m.base - j  # bord haut du pixel, en y vers le haut
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
            v = max(0.0, min(1.0, (coverage - LOW) / (HIGH - LOW)))
            row.append(round(v * LEVELS))
        rows.append(row)
    return rows


# --- Image PNG d'aperçu (sans bibliothèque) ---------------------------------------------------


def write_png(path, width, height, pixels):
    """Écrit une image en niveaux de gris 8 bits ; `pixels` est une liste de rangées d'octets."""

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    raw = b"".join(b"\x00" + bytes(row) for row in pixels)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 0, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)
