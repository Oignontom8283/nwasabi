//! Lecture ELF de tous les `.nwa` du corpus (si le corpus est présent : `NWASABI_CORPUS` ou `local/nwa`).

mod support;

use nwasabi::elf::{ElfObject, Limits};

#[test]
fn every_nwa_of_the_corpus_is_a_valid_arm_relocatable() {
    let Some(files) = support::corpus() else {
        return support::skip_without_corpus();
    };
    assert!(
        !files.is_empty(),
        "le dossier du corpus ne contient aucun .nwa"
    );
    for (name, bytes) in &files {
        let elf =
            ElfObject::parse(bytes, &Limits::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        // Les trois sections qu'exige le format d'une application externe.
        for required in [
            ".rodata.eadk_app_name",
            ".rodata.eadk_api_level",
            ".rodata.eadk_app_icon",
        ] {
            assert!(
                elf.section_by_name(required).is_some(),
                "{name}: section {required} absente"
            );
        }
        // Le point d'entrée C.
        assert!(
            elf.symbols
                .iter()
                .any(|s| s.name == "main" && s.is_defined()),
            "{name}: symbole main absent"
        );
        assert_eq!(
            elf.header.abi_version(),
            5,
            "{name}: ABI ARM EABI v5 attendue"
        );
    }
}
