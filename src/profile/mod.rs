//! Profil d'un firmware : tout ce qui change d'un modèle de calculatrice ou d'une version
//! d'Epsilon et que les applications peuvent observer (adresses des structures, versions,
//! identifiants…).
//!
//! Rien de tout cela ne doit être écrit en dur ailleurs dans nwasabi : l'adresse du
//! `UserlandHeader`, par exemple, dépend de la version du firmware et pas seulement du modèle.

mod firmware;
mod layout;
mod presets;

pub use firmware::{FirmwareProfile, ProfileError, Provenance};
pub use layout::{
    EXTERNAL_APP_SECTOR, EXTERNAL_FLASH_BASE, EXTERNAL_FLASH_SIZE, PERSISTING_BYTES_SIZE,
    SLOT_B_OFFSET, SlotLayout,
};

/// Modèle de calculatrice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Model {
    N0110,
    N0115,
    N0120,
}

impl Model {
    /// Nom commercial interne (`N0110`…).
    pub fn name(self) -> &'static str {
        match self {
            Self::N0110 => "N0110",
            Self::N0115 => "N0115",
            Self::N0120 => "N0120",
        }
    }

    /// Début de la SRAM, où se trouve le `SlotInfo`.
    pub fn sram_base(self) -> u32 {
        match self {
            Self::N0110 | Self::N0115 => 0x2000_0000,
            Self::N0120 => 0x2400_0000,
        }
    }

    /// Les trois modèles.
    pub const ALL: [Model; 3] = [Self::N0110, Self::N0115, Self::N0120];
}
