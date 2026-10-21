//! Le système de fichiers de la calculatrice, vu depuis l'hôte.
//!
//! Sur l'appareil, les fichiers vivent dans un tampon de RAM (voir [`records`]) que le système et
//! les applications lisent et modifient directement. nwasabi n'en garde donc **aucune copie** :
//! chaque opération relit le tampon dans la mémoire émulée, et les écritures de l'hôte le
//! modifient en place. L'hôte n'agit qu'entre deux appels à `Emulator::run`, donc jamais au
//! milieu d'une instruction : ses modifications sont atomiques pour l'application.

pub mod records;

use std::fmt;

use crate::memory::{Memory, MemoryError};

use records::Record;

/// Ce qui peut échouer en manipulant les fichiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    NotFound(String),
    Exists(String),
    /// Le nom doit être de la forme `nom.extension`, sans zéro.
    InvalidName(String),
    /// Le tampon est abîmé à cette position (souvent par l'application) : lecture de ce qui
    /// précède possible, écriture refusée.
    Corrupt {
        offset: usize,
    },
    /// Plus assez de place : `needed` octets demandés pour l'enregistrement.
    NoSpace {
        needed: usize,
        available: usize,
    },
    /// Un enregistrement ne peut pas dépasser 65 535 octets.
    TooLarge,
    Memory(MemoryError),
}

impl From<MemoryError> for FsError {
    fn from(e: MemoryError) -> Self {
        Self::Memory(e)
    }
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(n) => write!(f, "fichier introuvable : {n}"),
            Self::Exists(n) => write!(f, "le fichier existe déjà : {n}"),
            Self::InvalidName(n) => write!(f, "nom invalide (attendu nom.extension) : {n:?}"),
            Self::Corrupt { offset } => write!(f, "système de fichiers abîmé à l'octet {offset}"),
            Self::NoSpace { needed, available } => {
                write!(
                    f,
                    "plus de place : {needed} octets demandés, {available} libres"
                )
            }
            Self::TooLarge => write!(f, "fichier trop grand pour un enregistrement"),
            Self::Memory(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for FsError {}

/// Sorte de fichier, d'après son extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    /// Script Python (`.py`) : le corps est un octet « importé automatiquement » puis le texte.
    Python,
    /// Réglages du système (`.sys`) : à ne pas supprimer.
    System,
    /// Tout le reste (formats instables d'une version à l'autre) : traité comme des octets.
    Other(String),
}

impl FileKind {
    fn of(name: &str) -> Self {
        match name.rsplit_once('.').map(|(_, ext)| ext) {
            Some("py") => Self::Python,
            Some("sys") => Self::System,
            Some(ext) => Self::Other(ext.to_owned()),
            None => Self::Other(String::new()),
        }
    }
}

