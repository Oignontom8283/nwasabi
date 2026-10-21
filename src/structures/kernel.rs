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

/// Version et commit du noyau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelHeader {
    pub header: u32,
    /// Version d'Epsilon, par exemple `26.4.0`.
    pub software_version: String,
    /// Identifiant du commit de compilation.
    pub commit_hash: String,
    pub footer: u32,
}

impl KernelHeader {
    pub fn new(software_version: &str, commit_hash: &str) -> Self {
        Self {
            header: KERNEL_HEADER_MAGIC,
            software_version: software_version.to_owned(),
            commit_hash: commit_hash.to_owned(),
            footer: KERNEL_HEADER_MAGIC,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.header == KERNEL_HEADER_MAGIC && self.footer == KERNEL_HEADER_MAGIC
    }

    pub fn encode(&self) -> [u8; KERNEL_HEADER_SIZE] {
        let mut out = [0u8; KERNEL_HEADER_SIZE];
        put_word(&mut out, 0, self.header);
        put_text8(&mut out, 4, &self.software_version);
        put_text8(&mut out, 12, &self.commit_hash);
        put_word(&mut out, 20, self.footer);
        out
    }

    pub fn decode(bytes: &[u8; KERNEL_HEADER_SIZE]) -> Self {
        Self {
            header: word(bytes, 0),
            software_version: text8(bytes, 4),
            commit_hash: text8(bytes, 12),
            footer: word(bytes, 20),
        }
    }
}

