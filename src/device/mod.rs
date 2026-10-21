//! L'appareil émulé : sa mémoire, telle que l'OS la laisse, et les applications qu'on y installe.
//!
//! nwasabi n'exécute pas Epsilon ; il fabrique ce que celui-ci laisserait en mémoire une fois
//! démarré (en-têtes, système de fichiers vide, tables du userland) pour que les applications
//! qui regardent la mémoire y trouvent ce qu'elles attendent.

mod apps;
mod fabric;
mod map;

pub use apps::{AppEntry, list_apps};
pub use fabric::write_structures;
pub use map::build_memory;

use std::fmt;

use crate::link::{LayoutParams, LinkedApp};
use crate::memory::{Memory, MemoryError};
use crate::profile::{EXTERNAL_APP_SECTOR, FirmwareProfile, ProfileError};

/// Réglages de la fabrication d'un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeviceOptions {
    /// Valeur initiale de la RAM (0 par défaut). Sur un vrai appareil, la RAM contient ce qu'y
    /// ont laissé les programmes précédents ; certaines variables ne sont jamais mises à zéro.
    pub ram_fill: u8,
}

/// Ce qui peut échouer en construisant ou en modifiant l'appareil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceError {
    /// Le profil du firmware est incohérent.
    Profile(Vec<ProfileError>),
    Memory(MemoryError),
    /// L'application n'a pas été liée pour l'emplacement où elle serait installée.
    WrongPlace {
        expected: u32,
        found: u32,
    },
    /// Pas assez de place dans la zone des applications.
    NoRoom {
        needed: u32,
        available: u32,
    },
}

impl From<MemoryError> for DeviceError {
    fn from(e: MemoryError) -> Self {
        Self::Memory(e)
    }
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(errors) => {
                write!(f, "profil de firmware incohérent")?;
                for e in errors {
                    write!(f, " ; {e}")?;
                }
                Ok(())
            }
            Self::Memory(e) => write!(f, "{e}"),
            Self::WrongPlace { expected, found } => write!(
                f,
                "l'application est liée pour {found:#x} mais le prochain emplacement libre est {expected:#x}"
            ),
            Self::NoRoom { needed, available } => write!(
                f,
                "il faut {needed} octets et il n'en reste que {available} pour les applications"
            ),
        }
    }
}

impl std::error::Error for DeviceError {}

/// Un appareil : mémoire, profil et prochain emplacement libre pour une application.
pub struct DeviceImage {
    pub memory: Memory,
    pub profile: FirmwareProfile,
    /// Prochain emplacement où l'outil de flashage écrirait une application.
    next_app: u32,
}

impl DeviceImage {
    /// Construit l'appareil décrit par `profile` avec les réglages par défaut.
    pub fn new(profile: FirmwareProfile) -> Result<Self, DeviceError> {
        Self::with_options(profile, DeviceOptions::default())
    }

    pub fn with_options(
        profile: FirmwareProfile,
        options: DeviceOptions,
    ) -> Result<Self, DeviceError> {
        profile.validate().map_err(DeviceError::Profile)?;
        let mut memory = build_memory(&profile, options.ram_fill)?;
        write_structures(&mut memory, &profile)?;
        let next_app = profile.active().apps_flash.0;
        Ok(Self {
            memory,
            profile,
            next_app,
        })
    }

    /// Adresse où sera écrite la prochaine application installée.
    pub fn next_app_address(&self) -> u32 {
        self.next_app
    }

    /// Zones à donner à l'éditeur de liens pour une application qui sera installée ensuite.
    pub fn app_layout(&self) -> LayoutParams {
        let (_, end) = self.profile.active().apps_flash;
        LayoutParams {
            flash_start: self.next_app,
            flash_length: end.saturating_sub(self.next_app),
            ram_start: self.profile.apps_ram.0,
            ram_length: self.profile.apps_ram_len(),
            trampoline_address: self.profile.trampoline_address(),
        }
    }

    /// Écrit une application en flash, comme le ferait l'outil de flashage : à la suite des
    /// précédentes, complétée par des 0xFF jusqu'à la fin de son dernier secteur.
    ///
    /// L'application doit avoir été liée avec [`DeviceImage::app_layout`].
    pub fn install(&mut self, app: &LinkedApp) -> Result<u32, DeviceError> {
        if app.flash_start != self.next_app {
            return Err(DeviceError::WrongPlace {
                expected: self.next_app,
                found: app.flash_start,
            });
        }
        let (_, end) = self.profile.active().apps_flash;
        let available = end - self.next_app;
        if app.flash.len() as u32 > available {
            return Err(DeviceError::NoRoom {
                needed: app.flash.len() as u32,
                available,
            });
        }
        let address = self.next_app;
        self.memory.write_bytes(address, &app.flash)?;
        // Remplissage jusqu'au secteur suivant, sans sortir de la zone.
        let padded = app.installed_size().min(end - address);
        self.memory.fill(
            address + app.flash.len() as u32,
            padded - app.flash.len() as u32,
            0xff,
        )?;
        self.next_app = address + app.installed_size();
        debug_assert_eq!(self.next_app % EXTERNAL_APP_SECTOR, 0);
        Ok(address)
    }

    /// Efface la zone des applications externes (comme « supprimer toutes les applications »).
    pub fn erase_apps(&mut self) -> Result<(), DeviceError> {
        let (start, end) = self.profile.active().apps_flash;
        self.memory.fill(start, end - start, 0xff)?;
        self.next_app = start;
        Ok(())
    }

    /// Les applications installées, relues en mémoire (hors mode examen).
    pub fn apps(&mut self) -> Vec<AppEntry> {
        let (start, end) = self.profile.active().apps_flash;
        list_apps(&mut self.memory, start, end, false)
    }
}
