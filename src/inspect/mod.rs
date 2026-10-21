//! Inspection : relire, depuis la mémoire émulée, les structures de l'OS telles qu'une application
//! les verrait.
//!
//! Tout est lu **dans la mémoire**, pas dans la configuration : si une application (ou l'hôte)
//! écrase un en-tête, l'inspection le montre. Chaque structure est donc accompagnée de son
//! adresse et d'un indicateur de validité plutôt que d'être cachée quand elle est abîmée.

mod read;

pub use read::{detect_sram_base, read_structures};

use crate::device::AppEntry;
use crate::structures::{KernelHeader, SlotHeader, SlotInfo, UserlandHeader, VectorTable};

/// Une structure lue en mémoire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found<T> {
    /// Adresse où elle a été lue.
    pub address: u32,
    pub value: T,
    /// Les magics (et pointeurs) de la structure sont-ils en place ?
    pub valid: bool,
}

/// Famille de modèles que l'on peut distinguer d'après l'emplacement de la SRAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelClass {
    /// SRAM en `0x2000_0000` : N0110 ou N0115 (on ne les distingue pas par la mémoire).
    N0110OrN0115,
    /// SRAM en `0x2400_0000`.
    N0120,
}

/// Le système de fichiers en RAM, vu de l'extérieur : ses magics et la taille du premier
/// enregistrement. (La lecture des fichiers eux-mêmes viendra avec l'API de fichiers.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageView {
    pub address: u32,
    pub size: u32,
    pub header_magic_ok: bool,
    pub footer_magic_ok: bool,
    /// Taille du premier enregistrement (0 : le système de fichiers est vide).
    pub first_record_size: u16,
}

/// Tout ce que l'inspection retrouve en mémoire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Structures {
    pub sram_base: Option<u32>,
    pub model_class: Option<ModelClass>,
    pub slot_info: Option<Found<SlotInfo>>,
    pub slot_header: Option<Found<SlotHeader>>,
    pub kernel_header: Option<Found<KernelHeader>>,
    pub userland_header: Option<Found<UserlandHeader>>,
    pub vector_table: Option<Found<VectorTable>>,
    pub storage: Option<StorageView>,
    /// Nom de l'appareil, tel qu'écrit en flash (vide si aucun nom n'est défini).
    pub device_name: Option<String>,
    pub apps: Vec<AppEntry>,
}
