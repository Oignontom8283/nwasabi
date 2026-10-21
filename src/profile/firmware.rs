//! Le profil d'un firmware et ses contrôles de cohérence.

use std::fmt;

use super::Model;
use super::layout::{EXTERNAL_FLASH_BASE, EXTERNAL_FLASH_SIZE, SlotLayout};

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
    /// Identifiant du commit de compilation (7 caractères).
    pub commit_hash: String,
    /// Niveau de sécurité du slot (protection contre le retour à une version plus ancienne).
    pub security_level: u32,
    /// Taille de la charge signée du slot, à partir de l'octet 8.
    pub payload_length: u32,

    /// Slot actif (A) et son double (B).
    pub slot_a: SlotLayout,
    pub slot_b: SlotLayout,

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

    /// Identifiant FCC affiché dans « À propos ».
    pub fcc_id: String,
    pub pcb_version: u32,
    /// Numéro de série (16 caractères).
    pub serial_number: String,
    /// Nom de l'appareil (vide tant que l'utilisateur n'en a pas choisi).
    pub device_name: String,
}

