//! Outils partagés par les tests d'intégration.
//!
//! Chaque fichier de test n'en utilise qu'une partie.

#![allow(dead_code)]

use std::path::PathBuf;

/// Fichiers `.nwa` du corpus de test, s'il est fourni.
///
/// Le corpus (applications de tiers) n'est pas distribué avec le dépôt : on l'indique avec la
/// variable d'environnement `NWASABI_CORPUS` (dossier contenant des `.nwa`). Sans elle, les
/// tests qui en dépendent sont ignorés avec un message, et ne font pas échouer la suite.
pub fn corpus() -> Option<Vec<(String, Vec<u8>)>> {
    let dir = PathBuf::from(std::env::var_os("NWASABI_CORPUS")?);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("NWASABI_CORPUS={}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "nwa"))
        .collect();
    files.sort();
    Some(
        files
            .into_iter()
            .map(|p| {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                (name, std::fs::read(&p).unwrap())
            })
            .collect(),
    )
}

/// Message affiché quand un test est ignoré faute de corpus.
pub fn skip_without_corpus() {
    eprintln!("corpus absent (variable NWASABI_CORPUS non définie) : test ignoré");
}

use nwasabi::elf::{
    ObjectWriter, R_ARM_ABS32, R_ARM_THM_CALL, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SectionId,
    SymbolBind, SymbolId, SymbolKind, SymbolSection,
};
use nwasabi::link::LayoutParams;

/// Zones réservées aux applications sur un N0110 sous Epsilon 26.4.0 (valeurs lues dans le
/// `UserlandHeader` de ce firmware).
pub fn n0110_layout() -> LayoutParams {
    LayoutParams {
        flash_start: 0x9020_0000,
        flash_length: 0x1f_0000,
        ram_start: 0x2001_18a4,
        ram_length: 0x2003_7000 - 0x2001_18a4,
        trampoline_address: 0x9002_0038,
    }
}

