//! Empreinte CRC-32 (polynôme IEEE 802.3, comme zlib) : résume l'écran et les images dans les
//! références de test. L'encodage en image (PNG…) est l'affaire de l'hôte, pas du cœur.

/// CRC-32 (polynôme IEEE 802.3, comme zlib).
pub fn crc32(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }
}
