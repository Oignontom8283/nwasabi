//! nwasabi — cœur d'émulation des applications externes (fichiers `.nwa`) des calculatrices NumWorks.
//!
//! Le cœur est une bibliothèque pure : pas de threads, pas d'horloge système, pas d'entrées/sorties.
//! L'hôte (un outil en ligne de commande, une page web…) lui fournit les entrées et lit ses sorties.
//!
//! Le module [`cpu`] est autonome (il n'utilise jamais le reste de ce crate) pour pouvoir être
//! déplacé plus tard dans un crate séparé.

#![forbid(unsafe_code)]

pub mod cpu;
pub mod device;
pub mod elf;
pub mod emulator;
pub mod inspect;
pub mod kernel;
pub mod link;
pub mod memory;
pub mod profile;
pub mod settings;
pub mod storage;
pub mod structures;
