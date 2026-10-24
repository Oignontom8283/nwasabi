//! La police de `draw_string` : « Nwasabi Pixels », des glyphes en 16 niveaux de gris dans des
//! cellules fixes.
//!
//! Les dimensions des cellules (7×14 pixels en petit, 10×18 en grand) sont celles de l'appareil, pour
//! que la mise en page des applications reste la même. Les glyphes sont dessinés pour nwasabi : ils
//! ressemblent à ceux d'Epsilon sans en être des copies. Ils sont décrits comme des traits dans
//! `tools/font/` et rendus par `tools/font/generate.py`, qui écrit `data.rs`, un fichier source en
//! texte. La police est sous licence MIT (voir `LICENSE` dans ce dossier).
//!
//! Le texte est de l'UTF-8 (comme dans Epsilon) ; un caractère absent de la police est dessiné
//! comme un `?`.

// Fichier généré par tools/font/generate.py : rustfmt n'y touche pas.
#[rustfmt::skip]
mod data;

use data::{GLYPHS, Glyph};

/// Largeur et hauteur d'une cellule de caractère.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub width: u32,
    pub height: u32,
}

/// Cellule de la petite police.
pub const SMALL_CELL: Cell = Cell {
    width: data::SMALL_WIDTH,
    height: data::SMALL_HEIGHT,
};
/// Cellule de la grande police.
pub const LARGE_CELL: Cell = Cell {
    width: data::LARGE_WIDTH,
    height: data::LARGE_HEIGHT,
};

/// Niveau de gris maximal d'un pixel de glyphe.
pub const MAX_LEVEL: u8 = 15;

impl Cell {
    pub fn of(large: bool) -> Self {
        if large { LARGE_CELL } else { SMALL_CELL }
    }
}

/// Le glyphe de `code_point` (`?` s'il n'est pas dans la police).
fn glyph(code_point: u32) -> &'static Glyph {
    let find = |cp: u32| GLYPHS.binary_search_by_key(&cp, |g| g.code_point).ok();
    let index = find(code_point)
        .or_else(|| find(u32::from(b'?')))
        .expect("le point d'interrogation est dans la police");
    &GLYPHS[index]
}

/// Niveau de gris d'un caractère d'image : `.` éteint, `1`-`9` et `a`-`f` de 1 à 15.
fn level(c: u8) -> u8 {
    match c {
        b'1'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        _ => 0,
    }
}

/// Appelle `plot(x, y, niveau)` pour chaque pixel non nul du caractère, dans sa cellule (le coin
/// haut-gauche est `(0, 0)`). Le niveau va de 1 à [`MAX_LEVEL`].
pub fn draw_glyph(code_point: u32, large: bool, mut plot: impl FnMut(u32, u32, u8)) {
    let g = glyph(code_point);
    let image = if large { g.large } else { g.small };
    for (y, row) in image.split('/').enumerate() {
        for (x, c) in row.bytes().enumerate() {
            let level = level(c);
            if level != 0 {
                plot(x as u32, y as u32, level);
            }
        }
    }
}

/// Décode de l'UTF-8 ; une suite invalide donne U+FFFD, dessiné comme un `?`.
pub fn code_points(text: &[u8]) -> impl Iterator<Item = u32> + '_ {
    let mut rest = text;
    std::iter::from_fn(move || {
        let first = *rest.first()?;
        let length = match first {
            0x00..=0x7f => 1,
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => 1,
        };
        let decoded = rest
            .get(..length)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|s| s.chars().next());
        match decoded {
            Some(c) => {
                rest = &rest[length..];
                Some(u32::from(c))
            }
            None => {
                rest = &rest[1..];
                Some(0xfffd)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(c: u32, large: bool) -> Vec<(u32, u32, u8)> {
        let mut points = Vec::new();
        draw_glyph(c, large, |x, y, l| points.push((x, y, l)));
        points
    }

    #[test]
    fn the_cells_have_the_dimensions_of_the_device() {
        assert_eq!((SMALL_CELL.width, SMALL_CELL.height), (7, 14));
        assert_eq!((LARGE_CELL.width, LARGE_CELL.height), (10, 18));
    }

    #[test]
    fn space_draws_nothing_and_letters_do() {
        for large in [false, true] {
            assert!(lit(u32::from(b' '), large).is_empty());
            assert!(!lit(u32::from(b'A'), large).is_empty());
        }
    }

    #[test]
    fn pixels_stay_inside_the_cell_with_valid_levels() {
        for cp in GLYPHS.iter().map(|g| g.code_point) {
            for large in [false, true] {
                let cell = Cell::of(large);
                for (x, y, level) in lit(cp, large) {
                    assert!(x < cell.width && y < cell.height, "{cp:#x}");
                    assert!((1..=MAX_LEVEL).contains(&level));
                }
            }
        }
    }

    #[test]
    fn solid_strokes_reach_the_maximum_level() {
        assert!(lit(u32::from(b'l'), true).iter().any(|p| p.2 == MAX_LEVEL));
        assert!(lit(u32::from(b'H'), false).iter().any(|p| p.2 >= 12));
    }

    #[test]
    fn unknown_characters_look_like_a_question_mark() {
        assert_eq!(lit(0x1f600, false), lit(u32::from(b'?'), false));
        assert_eq!(lit(0xfffd, true), lit(u32::from(b'?'), true));
    }

    #[test]
    fn the_font_covers_latin_greek_and_math() {
        for cp in [
            'é', 'è', 'à', 'ç', 'œ', '€', '°', '±', '×', '÷', 'π', 'θ', 'μ', '≤', '≥', '≠', '∞',
            '√',
        ] {
            let found = GLYPHS
                .binary_search_by_key(&u32::from(cp), |g| g.code_point)
                .is_ok();
            assert!(found, "{cp}");
        }
        assert!(
            GLYPHS.windows(2).all(|w| w[0].code_point < w[1].code_point),
            "la liste est triée"
        );
    }

    #[test]
    fn utf8_is_decoded_and_invalid_bytes_are_replaced() {
        let decoded: Vec<u32> = code_points("aé€".as_bytes()).collect();
        assert_eq!(decoded, [0x61, 0xe9, 0x20ac]);
        let broken: Vec<u32> = code_points(&[0x61, 0xff, 0xc3, 0x62]).collect();
        assert_eq!(broken, [0x61, 0xfffd, 0xfffd, 0x62]);
        let cut: Vec<u32> = code_points(&[0xe2, 0x82]).collect();
        assert_eq!(cut, [0xfffd, 0xfffd]);
    }

    #[test]
    fn letters_differ() {
        assert_ne!(lit(u32::from(b'a'), false), lit(u32::from(b'b'), false));
        assert_ne!(lit(u32::from(b'A'), true), lit(u32::from(b'V'), true));
    }
}
