//! L'application liée, au format ELF exécutable : ce que GDB attend de `file application.elf`.
//!
//! Le débogueur y lit les symboles, les sections et les informations DWARF aux adresses définitives ; le code et les données
//! d'exécution, eux, viennent de la calculatrice (ou de Nwasabi) par la connexion `target remote`.

use super::{LinkedApp, SectionClass};
use crate::elf::SymbolKind;

const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_NOBITS: u32 = 8;
const SHF_WRITE: u32 = 1;
const SHF_ALLOC: u32 = 2;
const SHF_EXECINSTR: u32 = 4;

/// Une table de chaînes ELF qui évite les doublons.
#[derive(Default)]
struct Strings {
    bytes: Vec<u8>,
}

impl Strings {
    fn new() -> Self {
        Self { bytes: vec![0] }
    }

    fn add(&mut self, text: &str) -> u32 {
        if text.is_empty() {
            return 0;
        }
        let at = self.bytes.len() as u32;
        self.bytes.extend_from_slice(text.as_bytes());
        self.bytes.push(0);
        at
    }
}

struct Section {
    name: u32,
    kind: u32,
    flags: u32,
    address: u32,
    data: Vec<u8>,
    /// Taille d'une section sans contenu (`.bss`).
    nobits_size: u32,
    link: u32,
    info: u32,
    align: u32,
    entry_size: u32,
}

fn push16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

impl LinkedApp {
    /// Le fichier ELF de l'application : une section par section placée (`.text`, `.rodata`, `.data`, `.bss`), la table des symboles,
    /// et les sections de débogage.
    pub fn to_elf(&self) -> Vec<u8> {
        let mut names = Strings::new();
        let mut sections: Vec<Section> = vec![Section {
            name: 0,
            kind: 0,
            flags: 0,
            address: 0,
            data: Vec::new(),
            nobits_size: 0,
            link: 0,
            info: 0,
            align: 0,
            entry_size: 0,
        }];

        // Les sections de l'image, dans l'ordre des adresses.
        let mut placed: Vec<&super::PlacedSection> =
            self.sections.iter().filter(|s| s.size > 0).collect();
        placed.sort_by_key(|s| (s.address, s.name.clone()));
        let mut first_alloc = 0usize;
        for p in placed {
            let (kind, flags) = match p.class {
                SectionClass::Text | SectionClass::Header => {
                    (SHT_PROGBITS, SHF_ALLOC | SHF_EXECINSTR)
                }
                SectionClass::Rodata => (SHT_PROGBITS, SHF_ALLOC),
                SectionClass::Data => (SHT_PROGBITS, SHF_ALLOC | SHF_WRITE),
                SectionClass::Bss | SectionClass::Common => (SHT_NOBITS, SHF_ALLOC | SHF_WRITE),
            };
            let data = match (kind, p.flash_address) {
                (SHT_PROGBITS, Some(flash)) => {
                    let start = flash.wrapping_sub(self.flash_start) as usize;
                    self.flash
                        .get(start..start + p.size as usize)
                        .map(<[u8]>::to_vec)
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            };
            if first_alloc == 0 {
                first_alloc = sections.len();
            }
            sections.push(Section {
                name: names.add(&p.name),
                kind,
                flags,
                address: p.address,
                data,
                nobits_size: if kind == SHT_NOBITS { p.size } else { 0 },
                link: 0,
                info: 0,
                align: p.alignment.max(1),
                entry_size: 0,
            });
        }
        let image_count = sections.len();

        // Les informations de débogage (sans adresse).
        for d in &self.debug {
            sections.push(Section {
                name: names.add(&d.name),
                kind: SHT_PROGBITS,
                flags: 0,
                address: 0,
                data: d.data.clone(),
                nobits_size: 0,
                link: 0,
                info: 0,
                align: 1,
                entry_size: 0,
            });
        }

        // Les symboles : les locaux d'abord (obligatoire), puis les globaux.
        let mut symstr = Strings::new();
        let find_section = |address: u32| -> u16 {
            sections[1..image_count]
                .iter()
                .position(|s| {
                    let size = if s.kind == SHT_NOBITS {
                        s.nobits_size
                    } else {
                        s.data.len() as u32
                    };
                    address >= s.address && address < s.address + size.max(1)
                })
                .map_or(0xfff1, |i| (i + 1) as u16)
        };
        let mut symtab = Vec::new();
        symtab.extend_from_slice(&[0u8; 16]); // symbole nul
        let mut count = 1u32;
        let mut emit = |symtab: &mut Vec<u8>,
                        count: &mut u32,
                        name: &str,
                        value: u32,
                        size: u32,
                        kind: SymbolKind| {
            let st_type = match kind {
                SymbolKind::Func => 2u8,
                SymbolKind::Object => 1,
                _ => 0,
            };
            push32(symtab, symstr.add(name));
            push32(symtab, value);
            push32(symtab, size);
            symtab.push((1 << 4) | st_type); // global
            symtab.push(0);
            push16(symtab, find_section(value & !1));
            *count += 1;
        };
        let mut symbols: Vec<&super::LinkedSymbol> = self
            .symbols
            .iter()
            .filter(|s| !s.name.is_empty() && s.address != 0)
            .collect();
        symbols.sort_by_key(|s| (s.address, s.name.clone()));
        for s in symbols {
            // Une fonction Thumb porte le bit 0 à 1 dans la table des symboles.
            let value = if s.kind == SymbolKind::Func {
                s.address | 1
            } else {
                s.address
            };
            emit(&mut symtab, &mut count, &s.name, value, s.size, s.kind);
        }
        let strtab_index = sections.len() + 1;
        sections.push(Section {
            name: names.add(".symtab"),
            kind: SHT_SYMTAB,
            flags: 0,
            address: 0,
            data: symtab,
            nobits_size: 0,
            link: strtab_index as u32,
            info: 1, // un seul symbole local : le symbole nul
            align: 4,
            entry_size: 16,
        });
        sections.push(Section {
            name: names.add(".strtab"),
            kind: SHT_STRTAB,
            flags: 0,
            address: 0,
            data: symstr.bytes,
            nobits_size: 0,
            link: 0,
            info: 0,
            align: 1,
            entry_size: 0,
        });
        let shstrtab_name = names.add(".shstrtab");
        let shstrtab_index = sections.len();
        // Le contenu de .shstrtab est complet une fois son propre nom ajouté.
        sections.push(Section {
            name: shstrtab_name,
            kind: SHT_STRTAB,
            flags: 0,
            address: 0,
            data: names.bytes,
            nobits_size: 0,
            link: 0,
            info: 0,
            align: 1,
            entry_size: 0,
        });

        // Disposition du fichier : en-tête (52), contenu des sections, puis la table des sections.
        let mut out = vec![0u8; 52];
        let mut offsets = Vec::new();
        for s in &sections {
            let align = s.align.max(1) as usize;
            while !out.len().is_multiple_of(align) {
                out.push(0);
            }
            offsets.push(out.len() as u32);
            out.extend_from_slice(&s.data);
        }
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        let table = out.len() as u32;
        for (s, offset) in sections.iter().zip(&offsets) {
            push32(&mut out, s.name);
            push32(&mut out, s.kind);
            push32(&mut out, s.flags);
            push32(&mut out, s.address);
            push32(&mut out, if s.kind == 0 { 0 } else { *offset });
            push32(
                &mut out,
                if s.kind == SHT_NOBITS {
                    s.nobits_size
                } else {
                    s.data.len() as u32
                },
            );
            push32(&mut out, s.link);
            push32(&mut out, s.info);
            push32(&mut out, s.align);
            push32(&mut out, s.entry_size);
        }

        // En-tête ELF32 little-endian, ARM, exécutable.
        let mut header = Vec::with_capacity(52);
        header.extend_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1, 0]);
        header.extend_from_slice(&[0; 8]);
        push16(&mut header, 2); // ET_EXEC
        push16(&mut header, 40); // EM_ARM
        push32(&mut header, 1);
        push32(&mut header, self.entry);
        push32(&mut header, 0); // pas de table de segments
        push32(&mut header, table);
        push32(&mut header, 0x0500_0400); // EABI v5, flottants matériels
        push16(&mut header, 52);
        push16(&mut header, 0);
        push16(&mut header, 0);
        push16(&mut header, 40);
        push16(&mut header, sections.len() as u16);
        push16(&mut header, shstrtab_index as u16);
        out[..52].copy_from_slice(&header);
        out
    }
}
