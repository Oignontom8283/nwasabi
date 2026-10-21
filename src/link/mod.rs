//! Éditeur de liens : transforme un `.nwa` (objet ELF relogeable) en image prête à être placée
//! en mémoire, comme le fait l'outil de flashage officiel avant d'écrire l'application dans la
//! flash de la calculatrice.

pub mod runtime;
