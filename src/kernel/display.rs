//! L'écran : 320×240 pixels en RGB565, comme celui de la calculatrice.

use super::font::{self, Cell, MAX_LEVEL};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 240;

/// Rectangle en pixels, comme `eadk_rect_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Décode un rectangle passé dans deux registres : `r0 = x | y << 16`, `r1 = largeur | hauteur << 16`.
    pub fn from_registers(r0: u32, r1: u32) -> Self {
        Self {
            x: r0 & 0xffff,
            y: r0 >> 16,
            width: r1 & 0xffff,
            height: r1 >> 16,
        }
    }

    /// Nombre de pixels (la source ou la destination d'un transfert en contient autant).
    pub fn area(self) -> u32 {
        self.width * self.height
    }
}

/// Mélange deux couleurs RGB565 : `bg` pour le niveau 0, `fg` pour le niveau [`MAX_LEVEL`].
fn blend(bg: u16, fg: u16, level: u8) -> u16 {
    if level >= MAX_LEVEL {
        return fg;
    }
    let level = i32::from(level);
    let mix = |shift: u32, bits: u32| {
        let max = (1i32 << bits) - 1;
        let (b, f) = (i32::from(bg >> shift) & max, i32::from(fg >> shift) & max);
        ((b * (i32::from(MAX_LEVEL) - level) + f * level + i32::from(MAX_LEVEL) / 2)
            / i32::from(MAX_LEVEL)) as u16
    };
    mix(11, 5) << 11 | mix(5, 6) << 5 | mix(0, 5)
}

/// Le contenu de l'écran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
    pixels: Vec<u16>,
}

impl Default for Display {
    fn default() -> Self {
        Self::new()
    }
}

impl Display {
    /// Écran noir.
    pub fn new() -> Self {
        Self {
            pixels: vec![0; (WIDTH * HEIGHT) as usize],
        }
    }

    /// Les pixels, ligne par ligne, en RGB565.
    pub fn pixels(&self) -> &[u16] {
        &self.pixels
    }

    fn index(x: u32, y: u32) -> Option<usize> {
        (x < WIDTH && y < HEIGHT).then(|| (y * WIDTH + x) as usize)
    }

    pub fn get(&self, x: u32, y: u32) -> Option<u16> {
        Self::index(x, y).map(|i| self.pixels[i])
    }

    pub fn set(&mut self, x: u32, y: u32, color: u16) {
        if let Some(i) = Self::index(x, y) {
            self.pixels[i] = color;
        }
    }

    /// Recopie `source` (ligne par ligne, `rect.width` pixels par ligne) dans le rectangle ; ce
    /// qui dépasse de l'écran est ignoré.
    pub fn push(&mut self, rect: Rect, source: &[u16]) {
        for row in 0..rect.height {
            for col in 0..rect.width {
                if let Some(&color) = source.get((row * rect.width + col) as usize) {
                    self.set(rect.x + col, rect.y + row, color);
                }
            }
        }
    }

    /// Comme [`Display::push`], mais depuis des octets (pixels de 16 bits little-endian), ligne par
    /// ligne : la partie visible de chaque ligne est recopiée d'un seul bloc.
    pub fn push_bytes(&mut self, rect: Rect, source: &[u8]) {
        let stride = rect.width as usize * 2;
        let visible = (WIDTH.saturating_sub(rect.x).min(rect.width)) as usize;
        if visible == 0 {
            return;
        }
        for row in 0..rect.height as usize {
            let y = rect.y as usize + row;
            if y >= HEIGHT as usize {
                break;
            }
            let Some(line) = source.get(row * stride..row * stride + visible * 2) else {
                break;
            };
            let start = y * WIDTH as usize + rect.x as usize;
            for (pixel, bytes) in self.pixels[start..start + visible]
                .iter_mut()
                .zip(line.chunks_exact(2))
            {
                *pixel = u16::from_le_bytes([bytes[0], bytes[1]]);
            }
        }
    }

    /// Comme [`Display::pull`], mais en octets (pixels de 16 bits little-endian) ; ce qui est hors
    /// de l'écran vaut 0.
    pub fn pull_bytes(&self, rect: Rect) -> Vec<u8> {
        let mut out = vec![0u8; rect.area() as usize * 2];
        let visible = (WIDTH.saturating_sub(rect.x).min(rect.width)) as usize;
        if visible == 0 {
            return out;
        }
        for row in 0..rect.height as usize {
            let y = rect.y as usize + row;
            if y >= HEIGHT as usize {
                break;
            }
            let start = y * WIDTH as usize + rect.x as usize;
            let line = &mut out[row * rect.width as usize * 2..][..visible * 2];
            for (bytes, pixel) in line
                .chunks_exact_mut(2)
                .zip(&self.pixels[start..start + visible])
            {
                bytes.copy_from_slice(&pixel.to_le_bytes());
            }
        }
        out
    }

    pub fn fill(&mut self, rect: Rect, color: u16) {
        for row in 0..rect.height {
            for col in 0..rect.width {
                self.set(rect.x + col, rect.y + row, color);
            }
        }
    }

    /// Lit le rectangle ; les pixels hors de l'écran valent 0.
    pub fn pull(&self, rect: Rect) -> Vec<u16> {
        let mut out = Vec::with_capacity(rect.area() as usize);
        for row in 0..rect.height {
            for col in 0..rect.width {
                out.push(self.get(rect.x + col, rect.y + row).unwrap_or(0));
            }
        }
        out
    }

    /// Écrit du texte (UTF-8) à partir du point `(x, y)` : une cellule par caractère, sans retour à la
    /// ligne. Le fond de chaque cellule prend la couleur `bg` ; les glyphes, en 16 niveaux de gris,
    /// sont mélangés entre `bg` et `fg`.
    pub fn draw_text(&mut self, text: &[u8], x: u32, y: u32, large: bool, fg: u16, bg: u16) {
        let cell = Cell::of(large);
        for (i, code_point) in font::code_points(text).enumerate() {
            let left = x.saturating_add(i as u32 * cell.width);
            self.fill(
                Rect {
                    x: left,
                    y,
                    width: cell.width,
                    height: cell.height,
                },
                bg,
            );
            font::draw_glyph(code_point, large, |dx, dy, level| {
                self.set(left + dx, y + dy, blend(bg, fg, level));
            });
        }
    }

    /// L'écran en RGBA 8 bits par composante (pour l'afficher ou l'enregistrer).
    pub fn to_rgba(&self) -> Vec<u8> {
        let expand = |value: u16, bits: u32| {
            let v = u32::from(value);
            ((v << (8 - bits)) | (v >> (2 * bits - 8))) as u8
        };
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &p in &self.pixels {
            out.extend_from_slice(&[
                expand(p >> 11, 5),
                expand((p >> 5) & 0x3f, 6),
                expand(p & 0x1f, 5),
                255,
            ]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn rectangle_registers_are_decoded() {
        let r = Rect::from_registers(10 | 20 << 16, 30 | 40 << 16);
        assert_eq!(r, rect(10, 20, 30, 40));
    }

    #[test]
    fn fill_and_pull_round_trip() {
        let mut d = Display::new();
        d.fill(rect(5, 6, 3, 2), 0xf800);
        assert_eq!(d.pull(rect(4, 6, 5, 1)), [0, 0xf800, 0xf800, 0xf800, 0]);
        assert_eq!(d.get(5, 7), Some(0xf800));
        assert_eq!(d.get(5, 8), Some(0));
    }

    #[test]
    fn push_clips_at_the_screen_edge() {
        let mut d = Display::new();
        d.push(rect(318, 238, 4, 4), &[1; 16]);
        assert_eq!(d.get(319, 239), Some(1));
        assert_eq!(d.get(320, 240), None);
        assert_eq!(d.pixels().iter().filter(|&&p| p == 1).count(), 4);
    }

    #[test]
    fn pull_outside_the_screen_gives_zero() {
        let mut d = Display::new();
        d.fill(rect(0, 0, 320, 240), 7);
        assert_eq!(d.pull(rect(319, 239, 2, 2)), [7, 0, 0, 0]);
    }

    #[test]
    fn text_fills_the_cell_background_and_draws_the_glyph() {
        let mut d = Display::new();
        d.draw_text(b"A", 10, 10, false, 0xffff, 0x1234);
        assert_eq!(d.get(10, 10), Some(0x1234));
        assert!((0..14).any(|y| (0..7).any(|x| d.get(10 + x, 10 + y) == Some(0xffff))));
        assert_eq!(d.get(17, 10), Some(0));
    }

    #[test]
    fn utf8_text_takes_one_cell_per_character() {
        let mut a = Display::new();
        let mut b = Display::new();
        a.draw_text("é".as_bytes(), 0, 0, true, 0xffff, 0);
        b.draw_text(&[0xc3, 0xa9], 0, 0, true, 0xffff, 0);
        assert_eq!(a, b);
        // Un seul caractère : la cellule suivante (x = 10) est intacte.
        a.draw_text("é".as_bytes(), 0, 20, true, 0xffff, 0);
        assert!((0..18).all(|y| a.get(10, 20 + y) == Some(0)));
    }

    #[test]
    fn blending_goes_from_background_to_foreground() {
        assert_eq!(blend(0x0000, 0xffff, 0), 0x0000);
        assert_eq!(blend(0x0000, 0xffff, MAX_LEVEL), 0xffff);
        let middle = blend(0x0000, 0xffff, 8);
        assert!(middle > 0x0000 && middle < 0xffff);
        assert_eq!(
            blend(0x1234, 0x1234, 7),
            0x1234,
            "même couleur : rien ne change"
        );
    }

    #[test]
    fn byte_transfers_match_pixel_transfers() {
        let mut a = Display::new();
        let mut b = Display::new();
        let rect = rect(315, 237, 10, 5); // dépasse à droite et en bas
        let pixels: Vec<u16> = (0..rect.area() as u16).map(|i| i * 3 + 1).collect();
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
        a.push(rect, &pixels);
        b.push_bytes(rect, &bytes);
        assert_eq!(a, b);
        let pulled: Vec<u8> = a
            .pull(rect)
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(b.pull_bytes(rect), pulled);
        // Rectangle entièrement hors de l'écran.
        let far = Rect {
            x: 400,
            y: 0,
            width: 3,
            height: 2,
        };
        assert_eq!(a.pull_bytes(far), vec![0; 12]);
        b.push_bytes(far, &[1; 12]);
        assert_eq!(a, b);
    }

    #[test]
    fn rgba_expands_rgb565() {
        let mut d = Display::new();
        d.set(0, 0, 0xffff);
        d.set(1, 0, 0xf800);
        let rgba = d.to_rgba();
        assert_eq!(&rgba[0..4], [255, 255, 255, 255]);
        assert_eq!(&rgba[4..8], [255, 0, 0, 255]);
    }
}
