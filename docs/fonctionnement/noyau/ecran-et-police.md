---
title: "Écran et police"
description: "L'écran 320×240 en RGB565, les transferts de rectangles, le mélange des niveaux de gris de la police Nwasabi Pixels et le format de conversion vers RGBA."
---

# Écran et police

## L'écran

`Display` est un tableau de `320 × 240` pixels de 16 bits **RGB565** (5 bits de rouge, 6 de vert, 5 de bleu), ligne par ligne, noir au départ. Les trois appels EADK d'écran passent par un `Rect` décodé de deux
registres : `r0 = x | y << 16`, `r1 = largeur | hauteur << 16`.

| Appel | Méthode | Détail |
|---|---|---|
| 19 `push_rect` | `push_bytes(rect, octets)` | recopie `rect.width` pixels par ligne depuis un tampon de pixels de 16 bits petit-boutistes ; la partie visible de chaque ligne est recopiée d'un seul bloc ; ce qui dépasse de l'écran est ignoré |
| 18 `pull_rect` | `pull_bytes(rect)` | lit un rectangle ; ce qui est hors de l'écran vaut 0 |
| 20 `push_rect_uniform` | `fill(rect, couleur)` | remplit |

La version `Vec<u16>` (`push`, `pull`) sert aux tests et aux hôtes. `to_rgba()` produit l'écran en RGBA 8 bits (la conversion RGB565 → 8 bits recopie les bits hauts dans les bits bas), pour l'affichage et
l'enregistrement. L'écran de la page n'est jamais copié en JSON : `nwasabi_framebuffer_ptr` donne directement le tampon de 16 bits en mémoire WebAssembly ([Hôtes](../hotes/wasm.md)).

## La police `Nwasabi Pixels`

`kernel/font/` dessine le texte de `drawString` et de `_write`. La police est **originale** (licence MIT) : ses cellules ont les dimensions de celles de l'appareil, pour que la mise en page des applications soit la même, mais ses
glyphes ne copient pas ceux d'Epsilon.

- cellules fixes : **7×14** pixels en petite police, **10×18** en grande ;
- glyphes à **16 niveaux de gris** (`MAX_LEVEL = 15`) ; `font/data.rs` est un fichier source produit par `tools/font/generate.py` (sans binaire dans le dépôt) ;
- le texte est de l'**UTF-8** (comme dans Epsilon) ; un caractère absent de la police, ou une suite invalide, est dessiné comme `?` (`U+FFFD` si invalide) ;
- `draw_text(texte, x, y, grande, avant, fond)` remplit d'abord chaque cellule avec la couleur de fond, puis `plot(dx, dy, niveau)` pose chaque pixel allumé du glyphe, **mélangé** entre fond et avant-plan selon
  le niveau (`blend(bg, fg, level)` : interpolation linéaire de chaque composante RGB565). Pas de retour à la ligne : une cellule par caractère.

La police ne sert qu'aux **appels que le noyau traite**. Une application qui dessine son propre texte (la plupart) pousse des pixels avec `push_rect`.

## Dans le code

| Où | Quoi |
|---|---|
| [`Display`](code:src/kernel/display.rs#Display), [`push_bytes`](code:src/kernel/display.rs#push_bytes), [`pull_bytes`](code:src/kernel/display.rs#pull_bytes), [`fill`](code:src/kernel/display.rs#fill) | les trois transferts |
| [`Rect::from_registers`](code:src/kernel/display.rs#from_registers) | décodage des deux registres |
| [`to_rgba`](code:src/kernel/display.rs#to_rgba) | la conversion pour l'affichage |
| [`draw_text`](code:src/kernel/display.rs#draw_text), [`blend`](code:src/kernel/display.rs#blend) | le texte et le mélange |
| [`font/mod.rs`](code:src/kernel/font/mod.rs), [`draw_glyph`](code:src/kernel/font/mod.rs#draw_glyph) | la police |
| [`font/data.rs`](code:src/kernel/font/data.rs) | les glyphes (**fichier produit**, ne pas éditer à la main) |
| [`tools/font/`](code:tools/font/) | le générateur et la licence de la police |

## Pourquoi ainsi

- **RGB565 de bout en bout** : l'application envoie des pixels de 16 bits, l'écran les garde tels quels ; la conversion en RGBA n'a lieu qu'à l'affichage. Cela évite de convertir à chaque `push_rect` et garde l'écran **comparable bit à bit** (sommes de contrôle des tests).
- **Une police originale** : celle d'Epsilon est protégée ; les cellules (7×14 et 10×18) ont les mêmes **dimensions** pour que la mise en page des applications soit la même.
- **Les glyphes viennent d'un script** : le fichier source est généré (`tools/font/generate.py`), jamais édité à la main, pour qu'on puisse refaire la police sans binaire dans le dépôt.

## Pour modifier

- **Changer un glyphe** : modifier [`tools/font/`](code:tools/font/) puis régénérer `data.rs`.
- **Ajouter un appel d'écran** : [`handle`](code:src/kernel/svc.rs#handle) puis une méthode de `Display` ; garder la règle « ce qui dépasse de l'écran est ignoré », « ce qui est hors de l'écran se lit 0 ».
- **Tests** : les références d'écran des applications sont dans `tests/golden/screens.tsv` (corpus) ; une régression visuelle les fait échouer.
