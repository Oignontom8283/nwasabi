//! Erreurs de l'édition de liens.

use thiserror::Error;

use super::relocate::RelocError;
use crate::elf::ElfError;

/// Ce qui peut empêcher de lier une application.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LinkError {
    /// Le fichier `.nwa` n'est pas un objet ELF ARM lisible.
    #[error("fichier .nwa illisible : {0}")]
    Elf(#[from] ElfError),
    /// Une section exigée par le format d'application externe est absente.
    #[error("section obligatoire absente : {0}")]
    MissingSection(&'static str),
    /// `.rodata.eadk_api_level` doit contenir exactement un entier de 32 bits.
    #[error("eadk_api_level doit faire 4 octets, il en fait {size}")]
    InvalidApiLevel { size: u32 },
    /// Un symbole est utilisé mais défini nulle part.
    #[error("symbole non défini : {name} (utilisé par {referenced_from})")]
    UndefinedSymbol {
        name: String,
        referenced_from: String,
    },
    /// L'application utilise `eadk_external_data` mais aucune donnée externe n'a été fournie.
    #[error("l'application utilise eadk_external_data : il faut fournir des données externes")]
    MissingExternalData,
    /// Une relocation n'a pas pu être appliquée.
    #[error("relocation impossible dans {section}+{offset:#x} vers {symbol} : {error:?}")]
    Relocation {
        section: String,
        offset: u32,
        symbol: String,
        error: RelocError,
    },
    /// Un symbole actif désigne une section écartée (cas anormal).
    #[error("le symbole {symbol} désigne la section supprimée {section}")]
    DiscardedSection { symbol: String, section: String },
    /// Le programme ne tient pas dans la zone de flash disponible.
    #[error("l'application occupe {needed} octets de flash, {available} disponibles")]
    FlashOverflow { needed: u64, available: u32 },
    /// Les données de l'application ne tiennent pas dans la RAM disponible.
    #[error("l'application occupe {needed} octets de RAM, {available} disponibles")]
    RamOverflow { needed: u64, available: u32 },
    /// Fonctionnalité d'ELF non gérée (ex. variables locales aux threads).
    #[error("non géré : {0}")]
    Unsupported(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relocation_error_names_the_place_and_the_cause() {
        let error = LinkError::Relocation {
            section: ".text.main".into(),
            offset: 0x10,
            symbol: "f".into(),
            error: RelocError::OutOfSection,
        };
        assert_eq!(
            error.to_string(),
            "relocation impossible dans .text.main+0x10 vers f : OutOfSection"
        );
    }
}
