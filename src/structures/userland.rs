//! Le `UserlandHeader` et la table de vecteurs qui le suit.

use super::bytes::{put_text8, put_word, text8, word};

/// Magic du `UserlandHeader` (en tête et en queue).
pub const USERLAND_HEADER_MAGIC: u32 = 0xdec0_edfe;
/// Taille du `UserlandHeader` (version avec nom d'appareil, Epsilon 22 et suivants).
pub const USERLAND_HEADER_SIZE: usize = 48;
/// Décalage, depuis le `UserlandHeader`, de la table de fonctions du userland (la « trampoline » :
/// l'outil de flashage lit `drawString` à cet endroit pour les applications).
pub const TRAMPOLINE_TABLE_OFFSET: u32 = USERLAND_HEADER_SIZE as u32 + 8;

/// Ce que le userland publie pour les applications : où sont le système de fichiers, la zone
/// des applications externes et le nom de l'appareil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserlandHeader {
    pub header: u32,
    /// Version du noyau attendue par ce userland.
    pub expected_software_version: String,
    /// Adresse en RAM du système de fichiers et taille de son tampon.
    pub storage_address: u32,
    pub storage_size: u32,
    /// Zone de flash des applications externes `[début, fin)`.
    pub external_apps_flash: (u32, u32),
    /// Zone de RAM des applications externes `[début, fin)`.
    pub external_apps_ram: (u32, u32),
    /// Zone de flash du nom de l'appareil `[début, fin)`.
    pub device_name: (u32, u32),
    pub footer: u32,
}

impl UserlandHeader {
    pub fn is_valid(&self) -> bool {
        // Même contrôle que le userland : magics en place et aucun pointeur nul.
        self.header == USERLAND_HEADER_MAGIC
            && self.footer == USERLAND_HEADER_MAGIC
            && self.storage_address != 0
            && self.storage_size != 0
            && self.external_apps_flash.0 != 0
            && self.external_apps_flash.1 != 0
            && self.external_apps_ram.0 != 0
            && self.external_apps_ram.1 != 0
            && self.device_name.0 != 0
            && self.device_name.1 != 0
    }

    pub fn encode(&self) -> [u8; USERLAND_HEADER_SIZE] {
        let mut out = [0u8; USERLAND_HEADER_SIZE];
        put_word(&mut out, 0, self.header);
        put_text8(&mut out, 4, &self.expected_software_version);
        let words = [
            self.storage_address,
            self.storage_size,
            self.external_apps_flash.0,
            self.external_apps_flash.1,
            self.external_apps_ram.0,
            self.external_apps_ram.1,
            self.device_name.0,
            self.device_name.1,
            self.footer,
        ];
        for (i, value) in words.into_iter().enumerate() {
            put_word(&mut out, 12 + 4 * i, value);
        }
        out
    }

    pub fn decode(bytes: &[u8; USERLAND_HEADER_SIZE]) -> Self {
        Self {
            header: word(bytes, 0),
            expected_software_version: text8(bytes, 4),
            storage_address: word(bytes, 12),
            storage_size: word(bytes, 16),
            external_apps_flash: (word(bytes, 20), word(bytes, 24)),
            external_apps_ram: (word(bytes, 28), word(bytes, 32)),
            device_name: (word(bytes, 36), word(bytes, 40)),
            footer: word(bytes, 44),
        }
    }
}

