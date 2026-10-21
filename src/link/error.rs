//! Erreurs de l'édition de liens.

use std::fmt;

use super::relocate::RelocError;
use crate::elf::ElfError;

/// Ce qui peut empêcher de lier une application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// Le fichier `.nwa` n'est pas un objet ELF ARM lisible.
    Elf(ElfError),
    /// Une section exigée par le format d'application externe est absente.
    MissingSection(&'static str),
    /// `.rodata.eadk_api_level` doit contenir exactement un entier de 32 bits.
    InvalidApiLevel { size: u32 },
    /// Un symbole est utilisé mais défini nulle part.
    UndefinedSymbol {
        name: String,
        referenced_from: String,
    },
    /// L'application utilise `eadk_external_data` mais aucune donnée externe n'a été fournie.
    MissingExternalData,
    /// Une relocation n'a pas pu être appliquée.
    Relocation {
        section: String,
        offset: u32,
        symbol: String,
        error: RelocError,
    },
    /// Un symbole actif désigne une section écartée (cas anormal).
    DiscardedSection { symbol: String, section: String },
    /// Le programme ne tient pas dans la zone de flash disponible.
    FlashOverflow { needed: u64, available: u32 },
    /// Les données de l'application ne tiennent pas dans la RAM disponible.
    RamOverflow { needed: u64, available: u32 },
    /// Fonctionnalité d'ELF non gérée (ex. variables locales aux threads).
    Unsupported(&'static str),
}

impl From<ElfError> for LinkError {
    fn from(e: ElfError) -> Self {
        Self::Elf(e)
    }
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Elf(e) => write!(f, "fichier .nwa illisible : {e}"),
            Self::MissingSection(name) => write!(f, "section obligatoire absente : {name}"),
            Self::InvalidApiLevel { size } => {
                write!(f, "eadk_api_level doit faire 4 octets, il en fait {size}")
            }
            Self::UndefinedSymbol {
                name,
                referenced_from,
            } => {
                write!(
                    f,
                    "symbole non défini : {name} (utilisé par {referenced_from})"
                )
            }
            Self::MissingExternalData => write!(
                f,
                "l'application utilise eadk_external_data : il faut fournir des données externes"
            ),
            Self::Relocation {
                section,
                offset,
                symbol,
                error,
            } => write!(
                f,
                "relocation impossible dans {section}+{offset:#x} vers {symbol} : {error:?}"
            ),
            Self::DiscardedSection { symbol, section } => {
                write!(
                    f,
                    "le symbole {symbol} désigne la section supprimée {section}"
                )
            }
            Self::FlashOverflow { needed, available } => {
                write!(
                    f,
                    "l'application occupe {needed} octets de flash, {available} disponibles"
                )
            }
            Self::RamOverflow { needed, available } => {
                write!(
                    f,
                    "l'application occupe {needed} octets de RAM, {available} disponibles"
                )
            }
            Self::Unsupported(what) => write!(f, "non géré : {what}"),
        }
    }
}

impl std::error::Error for LinkError {}
