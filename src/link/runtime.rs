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

/// `_start` : copie `.data` de la flash vers la RAM, met `.bss` à zéro, appelle `main(0, NULL)`.
///
/// ```text
///  0: push {r4, lr}
///  2: ldr r0, =_data_section_start_ram      ; mot du pool à 0x30
///  4: ldr r1, =_data_section_start_flash    ; 0x34
///  6: ldr r2, =_data_section_end_ram        ; 0x38
///  8: cmp r0, r2 ; bcs 0x16                 ; boucle de copie
///  c: ldrb r3, [r1], #1 ; strb r3, [r0], #1 ; b 0x8
/// 16: ldr r0, =_bss_section_start_ram       ; 0x3c
/// 18: ldr r2, =_bss_section_end_ram         ; 0x40
/// 1a: movs r3, #0
/// 1c: cmp r0, r2 ; bcs 0x26                 ; boucle de mise à zéro
/// 20: strb r3, [r0], #1 ; b 0x1c
/// 26: movs r0, #0 ; movs r1, #0
/// 2a: bl main                               ; relocation R_ARM_THM_CALL
/// 2e: pop {r4, pc}
/// ```
const START_CODE: [u16; 24] = [
    0xb510, 0x480b, 0x490b, 0x4a0c, 0x4290, 0xd204, 0xf811, 0x3b01, 0xf800, 0x3b01, 0xe7f8, 0x4809,
    0x4a09, 0x2300, 0x4290, 0xd202, 0xf800, 0x3b01, 0xe7fa, 0x2000, 0x2100, 0xf7ff, 0xfffe, 0xbd10,
];
/// Décalage de l'instruction `bl main` dans `_start`.
const START_BL_OFFSET: u32 = 0x2a;
/// Symboles des cinq mots du pool de constantes de `_start` (à partir du décalage 0x30).
const START_POOL: [&str; 5] = [
    linker_symbols::DATA_START_RAM,
    linker_symbols::DATA_START_FLASH,
    linker_symbols::DATA_END_RAM,
    linker_symbols::BSS_START_RAM,
    linker_symbols::BSS_END_RAM,
];

/// `eadk_display_draw_string` : saute à la fonction dont l'adresse est stockée dans la table
/// du userland (la « trampoline »), exactement comme le runtime officiel.
///
/// ```text
/// 0: ldr.w ip, [pc, #8]    ; ip = _userland_trampoline_address (mot à 0xc)
/// 4: ldr.w ip, [ip]        ; ip = fonction du userland
/// 8: bx ip
/// a: nop
/// ```
const DRAW_STRING_CODE: [u16; 6] = [0xf8df, 0xc008, 0xf8dc, 0xc000, 0x4760, 0xbf00];

fn halfwords(code: &[u16]) -> Vec<u8> {
    code.iter().flat_map(|h| h.to_le_bytes()).collect()
}

fn add_function(w: &mut ObjectWriter, name: &str, code: &[u8]) -> crate::elf::SectionId {
    let section = w.add_section(&format!(".text.{name}"), SHF_ALLOC | SHF_EXECINSTR, 4, code);
    // Bit 0 à 1 : convention ARM pour une fonction Thumb.
    w.add_symbol(
        name,
        1,
        code.len() as u32,
        SymbolBind::Global,
        SymbolKind::Func,
        SymbolSection::Section(section.0),
    );
    section
}

/// Fonction qui ne fait que renvoyer `value` : `movs r0, #value` ou `mov.w r0, #-1`, puis `bx lr`.
fn constant_function(w: &mut ObjectWriter, name: &str, value: i32) {
    let mut code = match value {
        -1 => halfwords(&[0xf04f, 0x30ff]),
        0..=255 => halfwords(&[0x2000 | value as u16]),
        _ => unreachable!("constante non prévue"),
    };
    code.extend_from_slice(&0x4770u16.to_le_bytes()); // bx lr
    add_function(w, name, &code);
}

