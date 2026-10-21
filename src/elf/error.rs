//! Erreurs de lecture d'un fichier ELF.

use std::fmt;

/// Ce qui peut empêcher de lire un fichier ELF relogeable (`.o`, donc un `.nwa`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfError {
    /// Le fichier s'arrête avant la fin d'une structure ; `0` dit laquelle.
    Truncated(&'static str),
    /// Les quatre premiers octets ne sont pas `\x7fELF`.
    NotElf,
    /// Fichier ELF 64 bits (ou classe inconnue) : seul le 32 bits est géré.
    UnsupportedClass(u8),
    /// Fichier big-endian (ou ordre inconnu) : seul le little-endian est géré.
    UnsupportedEndianness(u8),
    /// Fichier qui n'est pas un objet relogeable (`ET_REL`) : valeur de `e_type`.
    UnsupportedType(u16),
    /// Fichier qui ne vise pas ARM (`EM_ARM`) : valeur de `e_machine`.
    UnsupportedMachine(u16),
    /// Le fichier n'a aucune section.
    NoSections,
    /// La table des sections ou l'une des sections sort du fichier.
    SectionOutOfBounds { index: usize },
    /// Un nom (de section ou de symbole) est mal formé ou hors de sa table de chaînes.
    InvalidString(&'static str),
    /// Un symbole désigne un indice de section réservé que l'on ne gère pas.
    UnsupportedSymbolSection { symbol: usize, index: u16 },
    /// Une table (sections, symboles, relocations) dépasse la limite demandée.
    TooMany {
        what: &'static str,
        count: usize,
        limit: usize,
    },
    /// Section de relocations avec addend explicite (`SHT_RELA`) : non géré (ARM utilise `SHT_REL`).
    RelaNotSupported { section: usize },
    /// Section de relocations, ou table de symboles, mal formée.
    Malformed { section: usize, what: &'static str },
}

impl fmt::Display for ElfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(what) => write!(f, "fichier ELF tronqué : {what}"),
            Self::NotElf => write!(f, "ce n'est pas un fichier ELF"),
            Self::UnsupportedClass(c) => {
                write!(f, "classe ELF {c} non gérée (seul le 32 bits l'est)")
            }
            Self::UnsupportedEndianness(e) => {
                write!(
                    f,
                    "ordre des octets {e} non géré (seul le little-endian l'est)"
                )
            }
            Self::UnsupportedType(t) => {
                write!(
                    f,
                    "type ELF {t} non géré (un .nwa est un objet relogeable, type 1)"
                )
            }
            Self::UnsupportedMachine(m) => {
                write!(f, "machine ELF {m} non gérée (ARM attendu, valeur 40)")
            }
            Self::NoSections => write!(f, "le fichier ELF ne contient aucune section"),
            Self::SectionOutOfBounds { index } => {
                write!(f, "la section {index} sort des limites du fichier")
            }
            Self::InvalidString(what) => write!(f, "chaîne invalide : {what}"),
            Self::UnsupportedSymbolSection { symbol, index } => write!(
                f,
                "le symbole {symbol} utilise l'indice de section réservé {index:#x}"
            ),
            Self::TooMany { what, count, limit } => {
                write!(f, "trop de {what} : {count} (limite {limit})")
            }
            Self::RelaNotSupported { section } => {
                write!(
                    f,
                    "la section {section} utilise des relocations RELA, non gérées"
                )
            }
            Self::Malformed { section, what } => {
                write!(f, "section {section} mal formée : {what}")
            }
        }
    }
}

impl std::error::Error for ElfError {}
