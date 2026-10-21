//! Zones de mémoire : description publique et stockage interne.

/// Taille d'une page de stockage : une zone n'alloue de mémoire que pour les pages touchées.
pub const PAGE_SIZE: u32 = 4096;
/// Taille d'une ligne de suivi : le suivi des accès se fait par blocs de 32 octets.
pub const LINE_SIZE: u32 = 32;

/// Ce que le programme émulé a le droit de faire dans une zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Perms {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Perms {
    /// Lecture et exécution : la flash vue du programme.
    pub const READ_EXECUTE: Self = Self {
        read: true,
        write: false,
        execute: true,
    };
    /// Lecture seule.
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        execute: false,
    };
    /// Lecture, écriture et exécution : la RAM.
    pub const READ_WRITE_EXECUTE: Self = Self {
        read: true,
        write: true,
        execute: true,
    };
    /// Aucun accès (mémoire réservée au noyau).
    pub const NONE: Self = Self {
        read: false,
        write: false,
        execute: false,
    };
}

/// Nature d'une zone, pour l'affichage et l'analyse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    Flash,
    Ram,
    /// Mémoire qui existe mais que le programme ne peut pas toucher (noyau).
    Reserved,
}

/// Description d'une zone de mémoire à créer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionSpec {
    pub name: String,
    pub base: u32,
    pub size: u32,
    pub perms: Perms,
    pub kind: RegionKind,
    /// Valeur des octets jamais écrits (0xFF pour une flash effacée).
    pub fill: u8,
}

/// Une zone telle que la voit l'extérieur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionInfo {
    pub name: String,
    pub base: u32,
    pub size: u32,
    pub perms: Perms,
    pub kind: RegionKind,
}

/// Indicateurs d'accès par ligne de 32 octets.
pub mod access {
    pub const READ: u8 = 1;
    pub const WRITE: u8 = 2;
    pub const EXECUTE: u8 = 4;
}

/// Une plage d'adresses qui a changé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangedRange {
    pub start: u32,
    pub len: u32,
}

