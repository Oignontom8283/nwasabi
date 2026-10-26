//! Le profil d'un firmware et ses contrôles de cohérence.

use std::fmt;

use super::Model;
use super::layout::{EXTERNAL_FLASH_BASE, EXTERNAL_FLASH_SIZE, Slot, SlotLayout};

/// Taille d'une chaîne de version ou de commit dans les en-têtes (octets, NUL compris).
pub const HEADER_STRING_SIZE: usize = 8;
/// Longueur du numéro de série copié par l'appel système correspondant.
pub const SERIAL_NUMBER_LEN: usize = 16;
/// Taille de la zone du nom de l'appareil (octets, NUL compris).
pub const DEVICE_NAME_SIZE: usize = 1024;

/// D'où viennent les valeurs d'un profil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Valeurs lues dans un vrai firmware.
    Measured { source: &'static str },
    /// Valeurs déduites d'un profil mesuré, en supposant la même disposition : à confirmer
    /// avec un vrai firmware du modèle visé.
    Derived { from: &'static str },
}

/// Tout ce qui varie entre modèles et versions et que les applications peuvent observer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareProfile {
    pub model: Model,
    pub provenance: Provenance,

    /// Version d'Epsilon (ex. `26.4.0`).
    pub software_version: String,
    /// Version du userland que le noyau attend, écrite dans le `UserlandHeader` (identique à
    /// `software_version` sur un appareil à jour ; elle diffère après une mise à jour partielle).
    pub expected_userland_version: String,
    /// Identifiant du commit de compilation (7 caractères).
    pub commit_hash: String,
    /// Niveau de sécurité du slot (protection contre le retour à une version plus ancienne).
    pub security_level: u32,
    /// Taille de la charge signée du slot, à partir de l'octet 8.
    pub payload_length: u32,

    /// Les deux slots (A et B), à des adresses décalées de la moitié de la flash.
    pub slot_a: SlotLayout,
    pub slot_b: SlotLayout,
    /// Slot que l'appareil exécute : c'est celui que désigne le `SlotInfo` et dont la zone des
    /// applications est publiée aux applications.
    pub active_slot: Slot,

    /// Taille de la SRAM.
    pub sram_size: u32,
    /// Taille de la partie de la SRAM réservée au noyau, en fin de SRAM.
    pub kernel_sram_size: u32,
    /// Adresse du système de fichiers en RAM (son magic de tête) et taille de son tampon.
    pub storage_address: u32,
    pub storage_size: u32,
    /// RAM utilisable par les applications externes `[début, fin)`.
    pub apps_ram: (u32, u32),
    /// Valeur initiale du pointeur de pile du userland (et donc de l'application).
    pub process_stack_start: u32,
    /// Valeur initiale du pointeur de pile du noyau.
    pub kernel_stack_start: u32,
    /// Fréquence du processeur en fonctionnement normal, en MHz (relevée dans le noyau : SysTick est
    /// réglé pour elle).
    pub cpu_mhz: u32,

    /// Identifiant FCC affiché dans « À propos ».
    pub fcc_id: String,
    pub pcb_version: u32,
    /// Numéro de série (16 caractères).
    pub serial_number: String,
    /// Nom de l'appareil (vide tant que l'utilisateur n'en a pas choisi).
    pub device_name: String,
}

/// Une incohérence dans un profil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// Une chaîne est trop longue ou non ASCII.
    BadString { field: &'static str, value: String },
    /// Une adresse ou une plage sort de la zone où elle devrait se trouver.
    OutOfBounds { field: &'static str },
    /// Deux plages qui ne doivent pas se chevaucher se chevauchent.
    Overlap {
        first: &'static str,
        second: &'static str,
    },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadString { field, value } => write!(f, "{field} invalide : {value:?}"),
            Self::OutOfBounds { field } => write!(f, "{field} hors de la zone attendue"),
            Self::Overlap { first, second } => write!(f, "{first} chevauche {second}"),
        }
    }
}

impl std::error::Error for ProfileError {}

impl FirmwareProfile {
    /// Début de la SRAM (adresse du `SlotInfo`).
    pub fn sram_base(&self) -> u32 {
        self.model.sram_base()
    }

    /// Début de la SRAM du noyau (inaccessible aux applications).
    pub fn kernel_sram_base(&self) -> u32 {
        self.sram_base() + self.sram_size - self.kernel_sram_size
    }

    /// Adresse du `SlotInfo` : la toute première adresse de la SRAM.
    pub fn slot_info_address(&self) -> u32 {
        self.sram_base()
    }

    /// Disposition du slot demandé.
    pub fn slot(&self, slot: Slot) -> &SlotLayout {
        match slot {
            Slot::A => &self.slot_a,
            Slot::B => &self.slot_b,
        }
    }

    /// Disposition du slot que l'appareil exécute.
    pub fn active(&self) -> &SlotLayout {
        self.slot(self.active_slot)
    }

    /// Adresse de la table de fonctions du userland (`UserlandHeader` + 0x38) du slot actif :
    /// l'outil de flashage la passe à l'éditeur de liens pour `eadk_display_draw_string`.
    pub fn trampoline_address(&self) -> u32 {
        self.active().userland_header + 0x38
    }

    /// Début et fin de la zone des applications externes du slot actif.
    pub fn apps_flash(&self) -> (u32, u32) {
        self.active().apps_flash
    }

    /// Zone de RAM des applications : début et taille.
    pub fn apps_ram_len(&self) -> u32 {
        self.apps_ram.1 - self.apps_ram.0
    }

    /// Vérifie la cohérence interne du profil.
    pub fn validate(&self) -> Result<(), Vec<ProfileError>> {
        let mut errors = Vec::new();
        let ascii = |s: &str, max: usize| s.is_ascii() && s.len() <= max && !s.contains('\0');
        for (field, value, max) in [
            (
                "software_version",
                &self.software_version,
                HEADER_STRING_SIZE - 1,
            ),
            (
                "expected_userland_version",
                &self.expected_userland_version,
                HEADER_STRING_SIZE - 1,
            ),
            ("commit_hash", &self.commit_hash, HEADER_STRING_SIZE - 1),
            ("fcc_id", &self.fcc_id, 31),
            ("device_name", &self.device_name, DEVICE_NAME_SIZE - 1),
        ] {
            if !ascii(value, max) {
                errors.push(ProfileError::BadString {
                    field,
                    value: value.clone(),
                });
            }
        }
        if !self.serial_number.is_ascii() || self.serial_number.len() != SERIAL_NUMBER_LEN {
            errors.push(ProfileError::BadString {
                field: "serial_number",
                value: self.serial_number.clone(),
            });
        }

        let flash_end = EXTERNAL_FLASH_BASE + EXTERNAL_FLASH_SIZE;
        for (name, slot) in [("slot_a", &self.slot_a), ("slot_b", &self.slot_b)] {
            let inside = |a: u32| (EXTERNAL_FLASH_BASE..flash_end).contains(&a);
            let ordered = slot.origin < slot.kernel_header
                && slot.kernel_header < slot.userland_header
                && slot.userland_header < slot.apps_flash.0
                && slot.apps_flash.0 < slot.apps_flash.1
                && slot.apps_flash.1 <= slot.persisting_bytes.0
                && slot.persisting_bytes.0 < slot.persisting_bytes.1
                && slot.persisting_bytes.1 <= flash_end;
            let name_inside = slot.device_name.0 >= slot.persisting_bytes.0
                && slot.device_name.0 < slot.device_name.1
                && slot.device_name.1 <= slot.persisting_bytes.1;
            if !(inside(slot.origin) && ordered && name_inside) {
                errors.push(ProfileError::OutOfBounds {
                    field: if name == "slot_a" { "slot_a" } else { "slot_b" },
                });
            }
        }
        if self.slot_a.persisting_bytes.1 > self.slot_b.origin {
            errors.push(ProfileError::Overlap {
                first: "slot_a",
                second: "slot_b",
            });
        }

        // RAM : SlotInfo, système de fichiers, RAM des applications, pile, partie du noyau.
        let base = self.sram_base();
        let sram_end = base + self.sram_size;
        let storage_end = self.storage_address + 4 + self.storage_size + 4;
        if self.storage_address < base + 16 || storage_end > self.apps_ram.0 {
            errors.push(ProfileError::OutOfBounds { field: "storage" });
        }
        if self.apps_ram.0 >= self.apps_ram.1 || self.apps_ram.1 > self.process_stack_start {
            errors.push(ProfileError::OutOfBounds { field: "apps_ram" });
        }
        if self.process_stack_start >= self.kernel_sram_base()
            || self.kernel_stack_start <= self.kernel_sram_base()
            || self.kernel_stack_start > sram_end
        {
            errors.push(ProfileError::OutOfBounds { field: "pile" });
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
