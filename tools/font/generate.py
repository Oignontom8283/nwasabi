#!/usr/bin/env python3
# Nwasabi Pixels : génération des glyphes de `draw_string`.
#
# Copyright (c) 2026 Oignontom8283 — licence MIT (voir `src/kernel/font/LICENSE`).
"""Génère la police « Nwasabi Pixels » de `draw_string`.

Usage :
    python3 tools/font/generate.py [--out src/kernel/font] [--preview apercu.png] [--text "Bonjour"]

Les glyphes sont dessinés dans `glyphs.py` comme des squelettes de traits, rastérisés en 16
niveaux de gris par `geometry.py` dans les cellules de l'appareil (7×14 et 10×18 pixels). Le
script écrit `data.rs`, un fichier source lisible : chaque glyphe y est une image en texte.
Aucune dépendance en dehors de la bibliothèque standard de Python.
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import glyphs  # noqa: E402
import glyphs_latin  # noqa: E402,F401  (enregistre les glyphes de la suite)
from geometry import LARGE, SIZES, SMALL, render, write_png  # noqa: E402

HEX = ".123456789abcdef"

# Points de code que la police doit couvrir : ASCII, Latin-1, quelques caractères du Latin étendu et
# de la typographie, le grec et des symboles mathématiques courants.
WANTED = sorted(
    set(range(0x20, 0x7F))
    | set(range(0xA0, 0x100))
    | {0x152, 0x153, 0x178, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2026, 0x20AC}
    | (set(range(0x391, 0x3AA)) - {0x3A2})
    | set(range(0x3B1, 0x3CA))
    | {0x2190, 0x2191, 0x2192, 0x2193, 0x2211, 0x221A, 0x221E, 0x2248, 0x2260, 0x2264, 0x2265}
)


def render_all():
    """Pour chaque point de code défini, les rangées rendues de chaque taille."""
    return {
        cp: {m.name: render(fn(m), m) for m in SIZES}
        for cp, fn in sorted(glyphs.GLYPHS.items())
    }


def preview(rendered, path, codes, zoom=6, columns=32):
    """Planche d'aperçu : chaque glyphe en petit puis en grand, agrandi `zoom` fois."""
    cell_w = max(SMALL.width, LARGE.width) * zoom + 4
    cell_h = (SMALL.height + LARGE.height) * zoom + 8
    rows_n = (len(codes) + columns - 1) // columns
    width, height = columns * cell_w, rows_n * cell_h
    img = [[255] * width for _ in range(height)]
    for n, cp in enumerate(codes):
        ox, oy = (n % columns) * cell_w + 2, (n // columns) * cell_h + 2
        y_off = 0
        for m in SIZES:
            rows = rendered[cp][m.name]
            for j, row in enumerate(rows):
                for i, level in enumerate(row):
                    shade = 255 - level * 17
                    for dy in range(zoom):
                        for dx in range(zoom):
                            if dx == 0 or dy == 0:
                                v = min(shade, 235)
                            else:
                                v = shade
                            img[oy + y_off + j * zoom + dy][ox + i * zoom + dx] = v
            y_off += m.height * zoom + 4
    write_png(path, width, height, img)


def text_preview(rendered, path, text, zoom=3):
    """Une ligne de texte, comme à l'écran : petite puis grande police."""
    img_rows, width = [], 0
    for m in SIZES:
        w = len(text) * m.width
        width = max(width, w)
    height = sum(m.height for m in SIZES) + 2
    img = [[255] * (width * zoom) for _ in range(height * zoom)]
    y_off = 0
    for m in SIZES:
        for n, ch in enumerate(text):
            rows = rendered.get(ord(ch), rendered[ord("?")])[m.name]
            for j, row in enumerate(rows):
                for i, level in enumerate(row):
                    for dy in range(zoom):
                        for dx in range(zoom):
                            img[(y_off + j) * zoom + dy][(n * m.width + i) * zoom + dx] = 255 - level * 17
        y_off += m.height + 2
    write_png(path, width * zoom, height * zoom, img)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", help="dossier où écrire data.rs")
    parser.add_argument("--preview", help="planche PNG de tous les glyphes")
    parser.add_argument("--text", help="avec --preview-text, la ligne à rendre")
    parser.add_argument("--preview-text", help="PNG d'une ligne de texte (--text)")
    parser.add_argument("--only", help="ne rendre que ces caractères")
    parser.add_argument("--zoom", type=int, default=6)
    parser.add_argument("--columns", type=int, default=32)
    args = parser.parse_args()

    rendered = render_all()
    codes = sorted(rendered)
    if args.only:
        codes = [ord(c) for c in args.only if ord(c) in rendered]
    if args.preview:
        preview(rendered, args.preview, codes, args.zoom, args.columns)
    if args.preview_text:
        text_preview(rendered, args.preview_text, args.text or "Hello")
    if args.out:
        write_data(rendered, os.path.join(args.out, "data.rs"))
    print(f"{len(rendered)} glyphes")


def image(rows):
    """Une image : les rangées en texte (`.` éteint, `1`-`f` niveaux), séparées par `/`."""
    return "/".join("".join(HEX[v] for v in row) for row in rows)


def write_data(rendered, path):
    missing = [cp for cp in WANTED if cp not in rendered]
    if missing:
        raise SystemExit("glyphes manquants : " + " ".join(f"U+{cp:04X}" for cp in missing))
    codes = [cp for cp in sorted(rendered) if cp in WANTED]
    lines = [
        "// Police « Nwasabi Pixels » (© 2026 Oignontom8283, licence MIT, voir `LICENSE` dans ce dossier).",
        "// Fichier généré par tools/font/generate.py : ne pas modifier à la main.",
        "//",
        "// Chaque glyphe est une image par taille : les rangées sont séparées par `/` ; `.` est un pixel",
        "// éteint, `1` à `9` et `a` à `f` les niveaux de gris de 1 à 15.",
        "",
        "pub const SMALL_WIDTH: u32 = %d;" % SMALL.width,
        "pub const SMALL_HEIGHT: u32 = %d;" % SMALL.height,
        "pub const LARGE_WIDTH: u32 = %d;" % LARGE.width,
        "pub const LARGE_HEIGHT: u32 = %d;" % LARGE.height,
        "",
        "/// Un caractère et ses deux images.",
        "pub struct Glyph {",
        "    pub code_point: u32,",
        "    pub small: &'static str,",
        "    pub large: &'static str,",
        "}",
        "",
        "/// Les glyphes, triés par point de code.",
        "pub static GLYPHS: [Glyph; %d] = [" % len(codes),
    ]
    for cp in codes:
        char = chr(cp)
        label = char if char.isprintable() and not char.isspace() else "(espace)"
        lines.append("    // %s" % label)
        lines.append(
            '    Glyph { code_point: 0x%04x, small: "%s", large: "%s" },'
            % (cp, image(rendered[cp]["small"]), image(rendered[cp]["large"]))
        )
    lines.append("];")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
