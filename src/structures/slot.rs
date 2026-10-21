//! Le `SlotInfo` : 16 octets au tout début de la SRAM qui désignent les en-têtes du slot actif.

use super::bytes::{put_word, word};

/// Magic du `SlotInfo` (en tête et en queue).
pub const SLOT_INFO_MAGIC: u32 = 0xefee_dbba;
/// Taille du `SlotInfo`.
pub const SLOT_INFO_SIZE: usize = 16;

/// Pointe vers le `KernelHeader` et le `UserlandHeader` du slot en cours d'exécution.
///
/// C'est le point d'entrée des applications qui cherchent des informations sur l'OS : il est
/// toujours à la première adresse de la SRAM, qui dépend du modèle (`0x2000_0000` ou
/// `0x2400_0000`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotInfo {
    pub header: u32,
    pub kernel_header: u32,
    pub userland_header: u32,
    pub footer: u32,
}

impl SlotInfo {
    pub fn new(kernel_header: u32, userland_header: u32) -> Self {
        Self {
            header: SLOT_INFO_MAGIC,
            kernel_header,
            userland_header,
            footer: SLOT_INFO_MAGIC,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.header == SLOT_INFO_MAGIC && self.footer == SLOT_INFO_MAGIC
    }

    pub fn encode(&self) -> [u8; SLOT_INFO_SIZE] {
        let mut out = [0u8; SLOT_INFO_SIZE];
        for (i, value) in [
            self.header,
            self.kernel_header,
            self.userland_header,
            self.footer,
        ]
        .into_iter()
        .enumerate()
        {
            put_word(&mut out, 4 * i, value);
        }
        out
    }

    pub fn decode(bytes: &[u8; SLOT_INFO_SIZE]) -> Self {
        Self {
            header: word(bytes, 0),
            kernel_header: word(bytes, 4),
            userland_header: word(bytes, 8),
            footer: word(bytes, 12),
        }
    }
}

