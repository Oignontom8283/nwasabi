//! Les structures de données que l'OS laisse en mémoire et que les applications lisent.
//!
//! Chacune sait s'écrire en octets (pour fabriquer l'image d'un appareil) et se relire à partir
//! d'octets (pour inspecter la mémoire telle que l'application la voit). Tous les entiers sont
//! little-endian. Les noms des champs suivent la terminologie d'Epsilon.

mod app;
mod bytes;
mod kernel;
mod slot;
mod userland;

pub use app::{APP_HEADER_SIZE, APP_MAGIC, AppHeader};
pub use kernel::{
    KERNEL_HEADER_MAGIC, KERNEL_HEADER_SIZE, KernelHeader, SLOT_HEADER_SIZE, SlotHeader,
};
pub use slot::{SLOT_INFO_MAGIC, SLOT_INFO_SIZE, SlotInfo};
pub use userland::{
    TRAMPOLINE_TABLE_OFFSET, USERLAND_HEADER_MAGIC, USERLAND_HEADER_SIZE, UserlandHeader,
    VectorTable,
};

/// Magic qui encadre le système de fichiers en RAM (avant et après son tampon).
pub const STORAGE_MAGIC: u32 = 0xee0b_ddba;
