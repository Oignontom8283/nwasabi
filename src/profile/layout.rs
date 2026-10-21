//! Disposition de la flash externe d'un appareil NumWorks (identique pour les trois modèles).

/// Début de la flash externe (8 Mio), où vivent les deux « slots » d'Epsilon.
pub const EXTERNAL_FLASH_BASE: u32 = 0x9000_0000;
/// Taille de la flash externe.
pub const EXTERNAL_FLASH_SIZE: u32 = 0x80_0000;
/// Décalage du slot B par rapport au slot A (la moitié de la flash).
pub const SLOT_B_OFFSET: u32 = 0x40_0000;
/// Taille d'un secteur de la flash externe : les applications y sont alignées.
pub const EXTERNAL_APP_SECTOR: u32 = 0x1_0000;
/// Taille de la zone des octets persistants (nom de l'appareil, état du mode examen).
pub const PERSISTING_BYTES_SIZE: u32 = 0x1_0000;

/// L'un des deux slots d'Epsilon. Un appareil exécute l'un ou l'autre : sur les appareils relevés,
/// le slot actif varie (voir `docs` : relevés matériel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    A,
    B,
}

/// Où se trouvent les structures d'un slot (A ou B) du firmware.
///
/// Un slot contient, dans cet ordre : un en-tête de 8 octets (niveau de sécurité, taille), le
/// noyau (dont le `KernelHeader` juste après l'en-tête), éventuellement une copie du bootloader,
/// le userland (dont le `UserlandHeader` en tête), puis la zone des applications externes, et
/// enfin les octets persistants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotLayout {
    /// Adresse du début du slot (en-tête de sécurité).
    pub origin: u32,
    /// Adresse du `KernelHeader`.
    pub kernel_header: u32,
    /// Adresse du `UserlandHeader`.
    pub userland_header: u32,
    /// Zone de flash réservée aux applications externes `[début, fin)`.
    pub apps_flash: (u32, u32),
    /// Zone du nom de l'appareil `[début, fin)`.
    pub device_name: (u32, u32),
    /// Zone des octets persistants (nom + état du mode examen).
    pub persisting_bytes: (u32, u32),
}

impl SlotLayout {
    /// Même disposition, décalée de `delta` octets (le slot B est le slot A décalé de [`SLOT_B_OFFSET`]).
    pub fn shifted(&self, delta: u32) -> Self {
        let sh = |(a, b): (u32, u32)| (a + delta, b + delta);
        Self {
            origin: self.origin + delta,
            kernel_header: self.kernel_header + delta,
            userland_header: self.userland_header + delta,
            apps_flash: sh(self.apps_flash),
            device_name: sh(self.device_name),
            persisting_bytes: sh(self.persisting_bytes),
        }
    }
}
