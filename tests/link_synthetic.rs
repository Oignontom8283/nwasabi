//! Tests de l'éditeur de liens sur de petites applications fabriquées à la main.

mod support;

use nwasabi::elf::{
    R_ARM_ABS32, SHF_ALLOC, SHF_TLS, SHF_WRITE, SymbolBind, SymbolKind, SymbolSection,
};
use nwasabi::link::thumb::decode_branch_offset;
use nwasabi::link::{
    APP_SECTOR_SIZE, LayoutParams, LinkError, LinkOptions, LinkWarning, LinkedApp, SectionClass,
    link,
};
use support::{
    API_LEVEL, CALL_SLOTS_AT, ICON, NAME, SyntheticApp, WORD_SLOTS_AT, n0110_layout, synthetic_app,
    synthetic_app_with, undefined,
};

fn link_with(app: SyntheticApp, layout: &LayoutParams) -> Result<LinkedApp, LinkError> {
    link(&app.finish(), None, layout, &LinkOptions::default())
}

fn link_app(app: SyntheticApp) -> Result<LinkedApp, LinkError> {
    link_with(app, &n0110_layout())
}

fn word(image: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(image[offset..offset + 4].try_into().unwrap())
}

fn symbol<'a>(app: &'a LinkedApp, name: &str) -> &'a nwasabi::link::LinkedSymbol {
    app.symbols
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("symbole {name} absent"))
}

/// Octets de l'image à partir d'une adresse (pour une fonction Thumb, retirer d'abord le bit 0).
fn bytes_at(app: &LinkedApp, address: u32, len: usize) -> &[u8] {
    let start = (address - app.flash_start) as usize;
    &app.flash[start..start + len]
}

#[test]
fn minimal_app_gets_a_valid_header() {
    let app = link_app(synthetic_app()).unwrap();
    assert_eq!(app.name, "Test");
    assert_eq!(app.api_level, 0);
    assert_eq!(app.icon, [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(app.flash_start, 0x9020_0000);

    // En-tête de 32 octets : magic, api, nom, taille icône, icône, entrée, fin, magic.
    let h = |i: usize| word(&app.flash, 4 * i);
    assert_eq!(h(0), 0xdec0_beba);
    assert_eq!(h(1), 0);
    assert_eq!(h(2), 0x20); // le nom suit l'en-tête
    assert_eq!(h(3), 8);
    assert_eq!(h(4), 0x20 + 5); // l'icône suit le nom (5 octets, alignement 1)
    assert_eq!(h(7), 0xdec0_beba);
    // L'entrée est l'adresse de `_start` sans son bit Thumb ; la « fin » est celle de `_eadk_app_end`.
    assert_eq!(h(5) & 1, 0);
    assert_eq!(h(5) + app.flash_start + 1, app.entry);
    assert_eq!(h(6) + app.flash_start, app.app_end);
}

#[test]
fn the_api_level_is_copied_into_the_header() {
    let app = synthetic_app_with(3u32.to_le_bytes().to_vec(), &[]);
    let linked = link_app(app).unwrap();
    assert_eq!(linked.api_level, 3);
    assert_eq!(word(&linked.flash, 4), 3);
}

#[test]
fn entry_point_is_the_start_stub_in_thumb_mode() {
    let app = link_app(synthetic_app()).unwrap();
    assert_eq!(symbol(&app, "_start").address, app.entry);
    assert_eq!(app.entry & 1, 1);
    // `_start` commence par `push {r4, lr}`.
    assert_eq!(bytes_at(&app, app.entry & !1, 2), [0x10, 0xb5]);
}

#[test]
fn unreferenced_sections_are_dropped() {
    let mut app = synthetic_app();
    app.function("unused");
    let linked = link_app(app).unwrap();
    assert!(linked.symbols.iter().all(|s| s.name != "unused"));
    assert!(linked.sections.iter().all(|s| s.name != ".text.unused"));
    // Les fonctions EADK que l'application n'appelle pas disparaissent aussi.
    assert!(linked.symbols.iter().all(|s| s.name != "eadk_random"));
}

#[test]
fn a_call_keeps_its_target_and_encodes_the_distance() {
    let mut app = synthetic_app();
    let callee = app.function("callee");
    let site = app.call(callee);
    let linked = link_app(app).unwrap();

    let main = symbol(&linked, "main").address & !1;
    let hw = |at: u32| u16::from_le_bytes(bytes_at(&linked, at, 2).try_into().unwrap());
    let at = main + site;
    let offset = decode_branch_offset(hw(at), hw(at + 2));
    let target = (i64::from(at) + 4 + i64::from(offset)) as u32;
    assert_eq!(target, symbol(&linked, "callee").address & !1);
    assert_eq!(site, CALL_SLOTS_AT);
}

#[test]
fn a_call_to_an_eadk_function_reaches_its_svc_stub() {
    let mut app = synthetic_app();
    let eadk = undefined(&mut app.writer, "eadk_random", SymbolBind::Global);
    let site = app.call(eadk);
    let linked = link_app(app).unwrap();

    let stub = symbol(&linked, "eadk_random").address;
    // `svc #45` puis `bx lr`.
    assert_eq!(bytes_at(&linked, stub & !1, 4), [45, 0xdf, 0x70, 0x47]);
    let at = (symbol(&linked, "main").address & !1) + site;
    let hw = |at: u32| u16::from_le_bytes(bytes_at(&linked, at, 2).try_into().unwrap());
    let offset = decode_branch_offset(hw(at), hw(at + 2));
    assert_eq!((i64::from(at) + 4 + i64::from(offset)) as u32, stub & !1);
}

#[test]
fn a_function_pointer_keeps_the_thumb_bit() {
    let mut app = synthetic_app();
    let callee = app.function("callee");
    let slot = app.word_ref(callee);
    let linked = link_app(app).unwrap();
    let main = symbol(&linked, "main").address & !1;
    let stored = u32::from_le_bytes(bytes_at(&linked, main + slot, 4).try_into().unwrap());
    assert_eq!(stored, symbol(&linked, "callee").address);
    assert_eq!(stored & 1, 1);
    assert_eq!(slot, WORD_SLOTS_AT);
}

