//! Erreurs de configuration et d'accès direct à la mémoire.

use std::fmt;

/// Ce qui peut échouer quand on configure la mémoire ou qu'on y accède depuis l'hôte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    /// Une zone doit commencer et se terminer sur une limite de page (4 Kio) et ne pas être vide.
    BadRegion { name: String, base: u32, size: u32 },
    /// La zone chevauche une zone déjà déclarée.
    Overlap { name: String, other: String },
    /// Une adresse (ou une partie de la plage demandée) n'est dans aucune zone.
    Unmapped { address: u32 },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRegion { name, base, size } => write!(
                f,
                "zone {name} invalide (début {base:#x}, taille {size:#x}) : début et taille doivent être des multiples de 4 Kio, taille non nulle"
            ),
            Self::Overlap { name, other } => {
                write!(f, "la zone {name} chevauche la zone {other}")
            }
            Self::Unmapped { address } => write!(f, "adresse {address:#010x} hors de toute zone"),
        }
    }
}

impl std::error::Error for MemoryError {}
