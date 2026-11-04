//! Erreurs de configuration et d'accès direct à la mémoire.

use thiserror::Error;

/// Ce qui peut échouer quand on configure la mémoire ou qu'on y accède depuis l'hôte.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MemoryError {
    /// Une zone doit commencer et se terminer sur une limite de page (4 Kio) et ne pas être vide.
    #[error(
        "zone {name} invalide (début {base:#x}, taille {size:#x}) : début et taille doivent être des multiples de 4 Kio, taille non nulle"
    )]
    BadRegion { name: String, base: u32, size: u32 },
    /// La zone chevauche une zone déjà déclarée.
    #[error("la zone {name} chevauche la zone {other}")]
    Overlap { name: String, other: String },
    /// Une adresse (ou une partie de la plage demandée) n'est dans aucune zone.
    #[error("adresse {address:#010x} hors de toute zone")]
    Unmapped { address: u32 },
}
