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

#[test]
fn an_undefined_symbol_is_only_an_error_when_something_uses_it() {
    // Référencé par une section que personne n'utilise : pas d'erreur.
    let mut unused = synthetic_app();
    let missing = undefined(&mut unused.writer, "does_not_exist", SymbolBind::Global);
    let dead = unused
        .writer
        .add_section(".rodata.dead", SHF_ALLOC, 4, &[0; 4]);
    unused.writer.add_relocation(dead, 0, missing, R_ARM_ABS32);
    assert!(link_app(unused).is_ok());

    // Référencé depuis `main` : erreur, avec le nom du symbole.
    let mut used = synthetic_app();
    let missing = undefined(&mut used.writer, "does_not_exist", SymbolBind::Global);
    used.word_ref(missing);
    match link_app(used) {
        Err(LinkError::UndefinedSymbol {
            name,
            referenced_from,
        }) => {
            assert_eq!(name, "does_not_exist");
            assert_eq!(referenced_from, ".text.main");
        }
        other => panic!("attendu UndefinedSymbol, obtenu {other:?}"),
    }
}

#[test]
fn a_weak_undefined_symbol_is_zero_and_a_call_to_it_becomes_a_nop() {
    let mut app = synthetic_app();
    let weak = undefined(&mut app.writer, "optional_hook", SymbolBind::Weak);
    let word_slot = app.word_ref(weak);
    let call_slot = app.call(weak);
    let linked = link_app(app).unwrap();
    let main = symbol(&linked, "main").address & !1;
    assert_eq!(bytes_at(&linked, main + word_slot, 4), [0, 0, 0, 0]);
    assert_eq!(
        bytes_at(&linked, main + call_slot, 4),
        [0xaf, 0xf3, 0x00, 0x80]
    ); // nop.w
}

#[test]
fn mandatory_header_sections_are_required() {
    for (skip, expected) in [
        (NAME, LinkError::MissingSection(NAME)),
        (ICON, LinkError::MissingSection(ICON)),
        (API_LEVEL, LinkError::MissingSection(API_LEVEL)),
    ] {
        let app = synthetic_app_with(vec![0; 4], &[skip]);
        assert_eq!(link_app(app).unwrap_err(), expected, "sans {skip}");
    }
    let app = synthetic_app_with(vec![0; 8], &[]);
    assert_eq!(
        link_app(app).unwrap_err(),
        LinkError::InvalidApiLevel { size: 8 }
    );
}

#[test]
fn files_that_are_not_arm_objects_are_rejected_cleanly() {
    let err = link(
        b"not an elf file at all",
        None,
        &n0110_layout(),
        &LinkOptions::default(),
    );
    assert!(matches!(err, Err(LinkError::Elf(_))), "{err:?}");
}

#[test]
fn data_image_follows_the_constants_without_padding() {
    let mut app = synthetic_app();
    let data = app.writer.add_section(
        ".data.counter",
        SHF_ALLOC | SHF_WRITE,
        4,
        &[0x11, 0x22, 0x33, 0x44],
    );
    let counter = app.writer.add_symbol(
        "counter",
        0,
        4,
        SymbolBind::Global,
        SymbolKind::Object,
        SymbolSection::Section(data.0),
    );
    // Une constante de 3 octets : la fin des constantes n'est alors pas alignée.
    let odd = app
        .writer
        .add_section(".rodata.odd", SHF_ALLOC, 1, &[1, 2, 3]);
    let odd = app.writer.add_symbol(
        "odd",
        0,
        3,
        SymbolBind::Global,
        SymbolKind::Object,
        SymbolSection::Section(odd.0),
    );
    let slot = app.word_ref(counter);
    app.word_ref(odd);
    let linked = link_app(app).unwrap();

    assert_eq!(linked.data.address, 0x2001_18a4); // début de la RAM des applications
    assert_eq!(linked.data.size, 4);
    assert_eq!(linked.data.flash_address, linked.app_end);
    assert_ne!(
        linked.app_end % 4,
        0,
        "le test suppose une fin de constantes non alignée"
    );
    assert_eq!(
        bytes_at(&linked, linked.data.flash_address, 4),
        [0x11, 0x22, 0x33, 0x44]
    );
    // Le pointeur vers `counter` contient son adresse en RAM.
    let main = symbol(&linked, "main").address & !1;
    assert_eq!(
        bytes_at(&linked, main + slot, 4),
        linked.data.address.to_le_bytes()
    );
    assert_eq!(symbol(&linked, "counter").address, linked.data.address);
}

#[test]
fn bss_is_in_ram_only_and_commons_follow_it() {
    let mut app = synthetic_app();
    let bss = app
        .writer
        .add_nobits(".bss.buffer", SHF_ALLOC | SHF_WRITE, 8, 16);
    let buffer = app.writer.add_symbol(
        "buffer",
        0,
        16,
        SymbolBind::Global,
        SymbolKind::Object,
        SymbolSection::Section(bss.0),
    );
    let shared = app.writer.add_symbol(
        "shared",
        4,
        12,
        SymbolBind::Global,
        SymbolKind::Common,
        SymbolSection::Common,
    );
    app.word_ref(buffer);
    app.word_ref(shared);
    let linked = link_app(app).unwrap();

    assert_eq!(linked.bss.size, 16);
    assert_eq!(linked.bss.address % 8, 0);
    assert_eq!(symbol(&linked, "buffer").address, linked.bss.address);
    // La variable commune est placée juste après la zone mise à zéro, et le tas commence après elle.
    let common = symbol(&linked, "shared");
    assert_eq!(common.address, linked.bss.address + linked.bss.size);
    assert_eq!(linked.heap.address, common.address + 12);
    assert_eq!(
        u64::from(linked.heap.address) + u64::from(linked.heap.size),
        n0110_layout().ram_end()
    );
    // Pas d'octets en flash pour .bss.
    assert!(
        linked
            .sections
            .iter()
            .any(|s| s.class == SectionClass::Bss && s.flash_address.is_none())
    );
}

