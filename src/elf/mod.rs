//! Lecture des fichiers ELF 32 bits ARM relogeables (`.o`).
//!
//! Un fichier `.nwa` est un tel fichier : ce n'est pas un programme prêt à être exécuté, il doit
//! encore être « lié » (voir le module `link`). Ce module se limite à lire le fichier de façon
//! sûre : toutes les positions et tailles sont vérifiées, jamais de `panic` sur une entrée
//! malformée.mod error;

pub use error::ElfError;
