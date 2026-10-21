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

