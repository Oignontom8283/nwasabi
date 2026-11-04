//! Erreurs de lecture d'un fichier ELF.

use thiserror::Error;

/// Ce qui peut empêcher de lire un fichier ELF relogeable (`.o`, donc un `.nwa`).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ElfError {
    /// Le fichier s'arrête avant la fin d'une structure ; `0` dit laquelle.
    #[error("fichier ELF tronqué : {0}")]
    Truncated(&'static str),
    /// Les quatre premiers octets ne sont pas `\x7fELF`.
    #[error("ce n'est pas un fichier ELF")]
    NotElf,
    /// Fichier ELF 64 bits (ou classe inconnue) : seul le 32 bits est géré.
    #[error("classe ELF {0} non gérée (seul le 32 bits l'est)")]
    UnsupportedClass(u8),
    /// Fichier big-endian (ou ordre inconnu) : seul le little-endian est géré.
    #[error("ordre des octets {0} non géré (seul le little-endian l'est)")]
    UnsupportedEndianness(u8),
    /// Fichier qui n'est pas un objet relogeable (`ET_REL`) : valeur de `e_type`.
    #[error("type ELF {0} non géré (un .nwa est un objet relogeable, type 1)")]
    UnsupportedType(u16),
    /// Fichier qui ne vise pas ARM (`EM_ARM`) : valeur de `e_machine`.
    #[error("machine ELF {0} non gérée (ARM attendu, valeur 40)")]
    UnsupportedMachine(u16),
    /// Le fichier n'a aucune section.
    #[error("le fichier ELF ne contient aucune section")]
    NoSections,
    /// La table des sections ou l'une des sections sort du fichier.
    #[error("la section {index} sort des limites du fichier")]
    SectionOutOfBounds { index: usize },
    /// Un nom (de section ou de symbole) est mal formé ou hors de sa table de chaînes.
    #[error("chaîne invalide : {0}")]
    InvalidString(&'static str),
    /// Un symbole désigne un indice de section réservé que l'on ne gère pas.
    #[error("le symbole {symbol} utilise l'indice de section réservé {index:#x}")]
    UnsupportedSymbolSection { symbol: usize, index: u16 },
    /// Une table (sections, symboles, relocations) dépasse la limite demandée.
    #[error("trop de {what} : {count} (limite {limit})")]
    TooMany {
        what: &'static str,
        count: usize,
        limit: usize,
    },
    /// Section de relocations avec addend explicite (`SHT_RELA`) : non géré (ARM utilise `SHT_REL`).
    #[error("la section {section} utilise des relocations RELA, non gérées")]
    RelaNotSupported { section: usize },
    /// Section de relocations, ou table de symboles, mal formée.
    #[error("section {section} mal formée : {what}")]
    Malformed { section: usize, what: &'static str },
}
