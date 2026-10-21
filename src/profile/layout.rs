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

