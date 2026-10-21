//! Le « runtime EADK » : le petit objet que l'outil de flashage de NumWorks lie avec chaque
//! application, et qui lui donne ses appels système.
//!
//! Sur la calculatrice, une application appelle par exemple `eadk_display_push_rect` ; cette
//! fonction n'existe pas dans le `.nwa` : elle vient d'un objet fourni à l'édition de liens,
//! fait de quelques instructions (`svc #N` puis retour). nwasabi fabrique un objet équivalent
//! pour que les appels de l'application arrivent, comme sur la vraie machine, sur une
//! instruction `svc` que le noyau simulé traite.
//!
//! L'objet fournit aussi `_start` (initialisation de `.data` et `.bss`, puis appel de `main`) et
//! les quelques fonctions de support de la bibliothèque C (`_sbrk`, `_write`…).
//!
//! Les instructions sont écrites ici en octets, avec leur désassemblage en commentaire ; elles
//! ont été vérifiées avec l'assembleur GNU pour ARM.

use crate::elf::{
    ObjectWriter, R_ARM_ABS32, R_ARM_THM_CALL, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SymbolBind,
    SymbolKind, SymbolSection,
};

/// Numéros d'appel système (`svc #N`) des fonctions EADK du firmware officiel, avec le nom de la
/// fonction C correspondante. Ces numéros sont ceux du noyau d'Epsilon.
pub const EADK_SVC: &[(&str, u8)] = &[
    ("eadk_backlight_brightness", 1),
    ("eadk_backlight_set_brightness", 2),
    ("eadk_display_pull_rect", 18),
    ("eadk_display_push_rect", 19),
    ("eadk_display_push_rect_uniform", 20),
    ("eadk_display_wait_for_vblank", 21),
    ("eadk_event_get", 23),
    ("eadk_keyboard_pop_state", 33),
    ("eadk_keyboard_scan", 34),
    ("eadk_random", 45),
    ("eadk_timing_millis", 48),
    ("eadk_timing_msleep", 49),
    ("eadk_timing_usleep", 50),
];

/// Fonctions déclarées dans `eadk.h` mais que le firmware officiel ne fournit pas aux
/// applications (un vrai appareil refuse de les lier). nwasabi peut les offrir en option.
pub const EADK_EXTENDED_SVC: &[(&str, u8)] = &[
    ("eadk_battery_is_charging", 3),
    ("eadk_battery_level", 4),
    ("eadk_battery_voltage", 5),
    ("eadk_usb_is_plugged", 52),
];

/// Appel système privé de nwasabi : `_sbrk` (agrandissement du tas).
pub const SVC_SBRK: u8 = 0x81;
/// Appel système privé de nwasabi : `_write` (affichage de texte).
pub const SVC_WRITE: u8 = 0x82;

/// Noms des symboles que définit l'éditeur de liens lui-même (comme le ferait un script de link).
pub mod linker_symbols {
    pub const TRAMPOLINE: &str = "_userland_trampoline_address";
    pub const HEAP_START: &str = "_heap_start";
    pub const HEAP_END: &str = "_heap_end";
    pub const DATA_START_FLASH: &str = "_data_section_start_flash";
    pub const DATA_START_RAM: &str = "_data_section_start_ram";
    pub const DATA_END_RAM: &str = "_data_section_end_ram";
    pub const BSS_START_RAM: &str = "_bss_section_start_ram";
    pub const BSS_END_RAM: &str = "_bss_section_end_ram";
    pub const APP_END: &str = "_eadk_app_end";
    pub const EXTERNAL_DATA_START: &str = "_eadk_external_data_start";
    pub const EXTERNAL_DATA_SIZE: &str = "_eadk_external_data_size";
}

