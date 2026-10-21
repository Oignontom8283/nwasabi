//! L'en-tête d'une application externe flashée.

use super::bytes::{put_word, word};

/// Magic de l'en-tête d'application (en tête et en queue).
pub const APP_MAGIC: u32 = 0xdec0_beba;
/// Taille de l'en-tête d'application.
pub const APP_HEADER_SIZE: usize = 32;

/// Les 32 premiers octets d'une application flashée, écrits par l'outil de flashage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppHeader {
    pub header: u32,
    pub api_level: u32,
    /// Position du nom (chaîne terminée par un zéro), depuis le début de l'application.
    pub name_offset: u32,
    pub icon_size: u32,
    pub icon_offset: u32,
    /// Position du point d'entrée (le système ajoute le bit Thumb).
    pub entry_offset: u32,
    /// Taille de l'application selon l'en-tête : fin des constantes, sans la copie de `.data`.
    pub size: u32,
    pub footer: u32,
}

impl AppHeader {
    pub fn is_valid(&self) -> bool {
        self.header == APP_MAGIC && self.footer == APP_MAGIC
    }

    pub fn encode(&self) -> [u8; APP_HEADER_SIZE] {
        let mut out = [0u8; APP_HEADER_SIZE];
        let words = [
            self.header,
            self.api_level,
            self.name_offset,
            self.icon_size,
            self.icon_offset,
            self.entry_offset,
            self.size,
            self.footer,
        ];
        for (i, value) in words.into_iter().enumerate() {
            put_word(&mut out, 4 * i, value);
        }
        out
    }

    pub fn decode(bytes: &[u8; APP_HEADER_SIZE]) -> Self {
        Self {
            header: word(bytes, 0),
            api_level: word(bytes, 4),
            name_offset: word(bytes, 8),
            icon_size: word(bytes, 12),
            icon_offset: word(bytes, 16),
            entry_offset: word(bytes, 20),
            size: word(bytes, 24),
            footer: word(bytes, 28),
        }
    }
}

