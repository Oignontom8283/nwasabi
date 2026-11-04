//! Le texte de chaque erreur de la bibliothèque : une instance de chaque variante, avec son message. Garde-fou des changements
//! d'écriture des erreurs (attributs, bibliothèque) : tant que le fichier de référence ne change pas, les messages sont les mêmes.
//!
//! Pour mettre à jour le fichier de référence après un changement voulu : `INSTA_UPDATE=always cargo test --test error_texts_golden`.

mod support;

use nwasabi::debuginfo::EvalError;
use nwasabi::device::DeviceError;
use nwasabi::elf::ElfError;
use nwasabi::emulator::EmulatorError;
use nwasabi::keymap::NmkError;
use nwasabi::link::LinkError;
use nwasabi::memory::MemoryError;
use nwasabi::profile::ProfileError;
use nwasabi::storage::FsError;

fn memory() -> Vec<MemoryError> {
    vec![
        MemoryError::BadRegion {
            name: "sram".into(),
            base: 0x2000_0001,
            size: 0x100,
        },
        MemoryError::Overlap {
            name: "a".into(),
            other: "b".into(),
        },
        MemoryError::Unmapped { address: 0x1234 },
    ]
}

fn profile() -> Vec<ProfileError> {
    vec![
        ProfileError::BadString {
            field: "serial",
            value: "é\n".into(),
        },
        ProfileError::OutOfBounds {
            field: "kernel_header",
        },
        ProfileError::Overlap {
            first: "a",
            second: "b",
        },
    ]
}

fn elf() -> Vec<ElfError> {
    vec![
        ElfError::Truncated("en-tête"),
        ElfError::NotElf,
        ElfError::UnsupportedClass(2),
        ElfError::UnsupportedEndianness(2),
        ElfError::UnsupportedType(2),
        ElfError::UnsupportedMachine(3),
        ElfError::NoSections,
        ElfError::SectionOutOfBounds { index: 7 },
        ElfError::InvalidString("nom de section"),
        ElfError::UnsupportedSymbolSection {
            symbol: 4,
            index: 0xfff1,
        },
        ElfError::TooMany {
            what: "sections",
            count: 70_000,
            limit: 65_535,
        },
        ElfError::RelaNotSupported { section: 3 },
        ElfError::Malformed {
            section: 5,
            what: "symboles",
        },
    ]
}

fn link() -> Vec<LinkError> {
    vec![
        LinkError::Elf(ElfError::NotElf),
        LinkError::MissingSection(".text"),
        LinkError::InvalidApiLevel { size: 3 },
        LinkError::UndefinedSymbol {
            name: "f".into(),
            referenced_from: ".text.main".into(),
        },
        LinkError::MissingExternalData,
        LinkError::DiscardedSection {
            symbol: "s".into(),
            section: ".x".into(),
        },
        LinkError::FlashOverflow {
            needed: 70_000,
            available: 65_536,
        },
        LinkError::RamOverflow {
            needed: 300_000,
            available: 262_144,
        },
        LinkError::Unsupported("TLS"),
    ]
}

fn fs() -> Vec<FsError> {
    vec![
        FsError::NotFound("a.py".into()),
        FsError::Exists("a.py".into()),
        FsError::InvalidName("sans extension".into()),
        FsError::Corrupt { offset: 12 },
        FsError::NoSpace {
            needed: 100,
            available: 10,
        },
        FsError::TooLarge,
        FsError::Memory(MemoryError::Unmapped { address: 4 }),
    ]
}

fn device() -> Vec<DeviceError> {
    vec![
        DeviceError::Profile(vec![]),
        DeviceError::Profile(profile()),
        DeviceError::Memory(MemoryError::Unmapped { address: 8 }),
        DeviceError::WrongPlace {
            expected: 0x9005_0000,
            found: 0x9006_0000,
        },
        DeviceError::NoRoom {
            needed: 100,
            available: 10,
        },
    ]
}

fn emulator() -> Vec<EmulatorError> {
    vec![
        EmulatorError::Link(LinkError::MissingExternalData),
        EmulatorError::Device(DeviceError::NoRoom {
            needed: 1,
            available: 0,
        }),
        EmulatorError::NoApp,
        EmulatorError::ExamMode,
        EmulatorError::Flash("trop grand".into()),
    ]
}

fn transcript() -> String {
    let mut out = String::new();
    let mut add = |kind: &str, items: Vec<String>| {
        for (i, text) in items.into_iter().enumerate() {
            out.push_str(&format!("{kind}[{i}] {text}\n"));
        }
    };
    add(
        "MemoryError",
        memory().iter().map(ToString::to_string).collect(),
    );
    add(
        "ProfileError",
        profile().iter().map(ToString::to_string).collect(),
    );
    add("ElfError", elf().iter().map(ToString::to_string).collect());
    add(
        "LinkError",
        link().iter().map(ToString::to_string).collect(),
    );
    add("FsError", fs().iter().map(ToString::to_string).collect());
    add(
        "DeviceError",
        device().iter().map(ToString::to_string).collect(),
    );
    add(
        "EmulatorError",
        emulator().iter().map(ToString::to_string).collect(),
    );
    add(
        "NmkError",
        vec![
            NmkError {
                path: String::new(),
                message: "fichier vide".into(),
            }
            .to_string(),
            NmkError {
                path: "touch.rules[1].gesture".into(),
                message: "geste inconnu".into(),
            }
            .to_string(),
        ],
    );
    add(
        "EvalError",
        vec![EvalError("variable inconnue : x".into()).to_string()],
    );
    out
}

#[test]
fn the_error_messages_are_exactly_as_recorded() {
    support::snapshot("errors", transcript());
}
