use super::*;

/// Un objet avec une fonction, des données, un `.bss`, des symboles de chaque sorte et deux relocations.
fn sample_object() -> Vec<u8> {
    let mut w = ObjectWriter::new();
    let text = w.add_section(
        ".text.main",
        SHF_ALLOC | SHF_EXECINSTR,
        4,
        &[0x70, 0x47, 0, 0],
    );
    let data = w.add_section(
        ".data.counter",
        SHF_ALLOC | SHF_WRITE,
        4,
        &[1, 0, 0, 0, 2, 0, 0, 0],
    );
    let bss = w.add_nobits(".bss.buffer", SHF_ALLOC | SHF_WRITE, 8, 64);
    let main = w.add_symbol(
        "main",
        1,
        4,
        SymbolBind::Global,
        SymbolKind::Func,
        SymbolSection::Section(text.0),
    );
    let counter = w.add_symbol(
        "counter",
        0,
        8,
        SymbolBind::Local,
        SymbolKind::Object,
        SymbolSection::Section(data.0),
    );
    w.add_symbol(
        "buffer",
        0,
        64,
        SymbolBind::Global,
        SymbolKind::Object,
        SymbolSection::Section(bss.0),
    );
    let external = w.add_symbol(
        "eadk_random",
        0,
        0,
        SymbolBind::Global,
        SymbolKind::NoType,
        SymbolSection::Undefined,
    );
    w.add_symbol(
        "limit",
        0x1234,
        0,
        SymbolBind::Global,
        SymbolKind::NoType,
        SymbolSection::Absolute,
    );
    w.add_symbol(
        "shared",
        4,
        16,
        SymbolBind::Global,
        SymbolKind::Common,
        SymbolSection::Common,
    );
    w.add_relocation(text, 0, external, R_ARM_THM_CALL);
    w.add_relocation(data, 4, main, R_ARM_ABS32);
    w.add_relocation(data, 0, counter, R_ARM_ABS32);
    w.finish()
}

#[test]
fn parses_sections_with_their_names() {
    let bytes = sample_object();
    let elf = ElfObject::parse(&bytes, &Limits::default()).unwrap();
    assert_eq!(elf.header.abi_version(), 5);
    let text = elf.section_by_name(".text.main").expect("section de code");
    assert!(elf.sections[text].is_executable() && elf.sections[text].is_alloc());
    assert_eq!(elf.section_data(text), &[0x70, 0x47, 0, 0]);
    let data = elf.section_by_name(".data.counter").unwrap();
    assert!(elf.sections[data].is_writable());
    assert_eq!(elf.section_data(data).len(), 8);
}

#[test]
fn nobits_sections_have_a_size_but_no_content() {
    let bytes = sample_object();
    let elf = ElfObject::parse(&bytes, &Limits::default()).unwrap();
    let bss = elf.section_by_name(".bss.buffer").unwrap();
    assert!(elf.sections[bss].is_nobits());
    assert_eq!(elf.sections[bss].size, 64);
    assert_eq!(elf.sections[bss].alignment(), 8);
    assert!(elf.section_data(bss).is_empty());
}

#[test]
fn parses_symbols_of_every_kind() {
    let bytes = sample_object();
    let elf = ElfObject::parse(&bytes, &Limits::default()).unwrap();
    let find = |name: &str| elf.symbols.iter().find(|s| s.name == name).unwrap();

    let main = find("main");
    assert!(main.is_thumb_function() && main.is_defined() && !main.is_weak());
    assert_eq!(find("counter").bind, SymbolBind::Local);
    assert_eq!(find("eadk_random").section, SymbolSection::Undefined);
    assert_eq!(find("limit").section, SymbolSection::Absolute);
    assert_eq!(find("limit").value, 0x1234);
    let shared = find("shared");
    assert_eq!(
        (shared.section, shared.value, shared.size),
        (SymbolSection::Common, 4, 16)
    );
    // Le symbole d'indice 0 est toujours le symbole nul.
    assert_eq!(elf.symbols[0].name, "");
}

#[test]
fn relocations_are_grouped_by_target_section() {
    let bytes = sample_object();
    let elf = ElfObject::parse(&bytes, &Limits::default()).unwrap();
    let text = elf.section_by_name(".text.main").unwrap();
    let data = elf.section_by_name(".data.counter").unwrap();
    assert_eq!(elf.relocations(text).len(), 1);
    assert_eq!(elf.relocations(data).len(), 2);
    let call = elf.relocations(text)[0];
    assert_eq!((call.offset, call.kind), (0, R_ARM_THM_CALL));
    assert_eq!(elf.symbols[call.symbol as usize].name, "eadk_random");
    let to_main = elf
        .relocations(data)
        .iter()
        .find(|r| r.offset == 4)
        .unwrap();
    assert_eq!(elf.symbols[to_main.symbol as usize].name, "main");
}

#[test]
fn rejects_non_elf_and_unsupported_files() {
    let good = sample_object();
    let limits = Limits::default();
    assert_eq!(
        ElfObject::parse(b"MZ not an elf file at all, long enough", &limits).unwrap_err(),
        ElfError::NotElf
    );

    let mut elf64 = good.clone();
    elf64[4] = 2;
    assert_eq!(
        ElfObject::parse(&elf64, &limits).unwrap_err(),
        ElfError::UnsupportedClass(2)
    );

    let mut big_endian = good.clone();
    big_endian[5] = 2;
    assert_eq!(
        ElfObject::parse(&big_endian, &limits).unwrap_err(),
        ElfError::UnsupportedEndianness(2)
    );

    let mut executable = good.clone();
    executable[16] = 2; // ET_EXEC
    assert_eq!(
        ElfObject::parse(&executable, &limits).unwrap_err(),
        ElfError::UnsupportedType(2)
    );

    let mut x86 = good.clone();
    x86[18] = 62;
    assert_eq!(
        ElfObject::parse(&x86, &limits).unwrap_err(),
        ElfError::UnsupportedMachine(62)
    );
}

#[test]
fn truncated_files_never_panic() {
    let good = sample_object();
    for len in 0..good.len() {
        // Chaque préfixe doit donner une erreur propre (ou, rarement, un objet valide), jamais un panic.
        let _ = ElfObject::parse(&good[..len], &Limits::default());
    }
}

#[test]
fn corrupted_files_never_panic() {
    let good = sample_object();
    for i in 0..good.len() {
        let mut bad = good.clone();
        bad[i] ^= 0xff;
        let _ = ElfObject::parse(&bad, &Limits::default());
    }
}

#[test]
fn limits_are_enforced() {
    let bytes = sample_object();
    let tiny = Limits {
        max_relocations: 1,
        ..Limits::default()
    };
    assert!(matches!(
        ElfObject::parse(&bytes, &tiny),
        Err(ElfError::TooMany {
            what: "relocations",
            ..
        })
    ));
    let small_file = Limits {
        max_file_size: 10,
        ..Limits::default()
    };
    assert!(matches!(
        ElfObject::parse(&bytes, &small_file),
        Err(ElfError::TooMany { what: "octets", .. })
    ));
}
