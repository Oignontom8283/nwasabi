//! En-tête de sécurité du slot et `KernelHeader`.

use super::bytes::{put_text8, put_word, text8, word};

/// Magic du `KernelHeader` (en tête et en queue).
pub const KERNEL_HEADER_MAGIC: u32 = 0xdec0_0df0;
/// Taille du `KernelHeader`.
pub const KERNEL_HEADER_SIZE: usize = 24;
/// Taille de l'en-tête de sécurité qui précède le `KernelHeader`.
pub const SLOT_HEADER_SIZE: usize = 8;

/// Les 8 premiers octets d'un slot : protection contre le retour en arrière et taille signée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotHeader {
    /// Niveau de sécurité : un firmware de niveau plus bas que celui installé est refusé.
    pub security_level: u32,
    /// Taille de la charge signée (noyau et userland) qui suit ces 8 octets.
    pub payload_length: u32,
}

impl SlotHeader {
    pub fn encode(&self) -> [u8; SLOT_HEADER_SIZE] {
        let mut out = [0u8; SLOT_HEADER_SIZE];
        put_word(&mut out, 0, self.security_level);
        put_word(&mut out, 4, self.payload_length);
        out
    }

    pub fn decode(bytes: &[u8; SLOT_HEADER_SIZE]) -> Self {
        Self {
            security_level: word(bytes, 0),
            payload_length: word(bytes, 4),
        }
    }
}

