//! Petites fonctions de lecture et d'écriture d'octets partagées par les structures.

/// Mot de 32 bits little-endian à la position `at`.
pub(super) fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

pub(super) fn put_word(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Chaîne de 8 octets terminée par des zéros, comme dans les en-têtes d'Epsilon.
pub(super) fn text8(bytes: &[u8], at: usize) -> String {
    let field = &bytes[at..at + 8];
    let end = field.iter().position(|&b| b == 0).unwrap_or(8);
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// Écrit `text` (au plus 8 octets, le reste est complété par des zéros).
pub(super) fn put_text8(out: &mut [u8], at: usize, text: &str) {
    let bytes = text.as_bytes();
    let n = bytes.len().min(8);
    out[at..at + 8].fill(0);
    out[at..at + n].copy_from_slice(&bytes[..n]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_round_trip_little_endian() {
        let mut b = [0u8; 8];
        put_word(&mut b, 2, 0x0102_0304);
        assert_eq!(b, [0, 0, 4, 3, 2, 1, 0, 0]);
        assert_eq!(word(&b, 2), 0x0102_0304);
    }

    #[test]
    fn text_fields_are_zero_padded_and_truncated() {
        let mut b = [0xaau8; 12];
        put_text8(&mut b, 2, "26.4.0");
        assert_eq!(&b[2..10], b"26.4.0\0\0");
        assert_eq!(text8(&b, 2), "26.4.0");
        put_text8(&mut b, 2, "0123456789");
        assert_eq!(&b[2..10], b"01234567");
        assert_eq!(text8(&b, 2), "01234567"); // sans zéro final : tout le champ
        assert_eq!(b[10], 0xaa);
    }
}
