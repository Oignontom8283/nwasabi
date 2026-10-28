//! nwasabi — cœur d'émulation des applications externes (fichiers `.nwa`) des calculatrices NumWorks.
//!
//! Le cœur est une bibliothèque pure : pas de threads, pas d'horloge système, pas d'entrées/sorties.
//! L'hôte (un outil en ligne de commande, une page web…) lui fournit les entrées et lit ses sorties.
//!
//! Le module [`cpu`] est autonome (il n'utilise jamais le reste de ce crate) pour pouvoir être
//! déplacé plus tard dans un crate séparé.

// Le cœur n'a pas de code `unsafe`, sauf les quelques fonctions d'échange avec JavaScript du module
// `wasm` (feature du même nom) : la mémoire d'un module WebAssembly ne se partage que par pointeurs.
#![cfg_attr(not(feature = "wasm"), forbid(unsafe_code))]
#![cfg_attr(feature = "wasm", deny(unsafe_code))]

pub mod checksum;
pub mod cpu;
pub mod debuginfo;
pub mod device;
pub mod elf;
pub mod emulator;
pub mod formats;
#[cfg(not(target_arch = "wasm32"))]
pub mod gdb;
pub mod inspect;
pub mod json;
pub mod kernel;
pub mod keymap;
pub mod link;
pub mod memory;
pub mod profile;
pub mod scb;
pub mod session;
pub mod settings;
pub mod storage;
pub mod structures;
#[cfg(feature = "wasm")]
pub mod wasm;
