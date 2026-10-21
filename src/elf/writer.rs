//! Écriture d'un fichier ELF 32 bits ARM relogeable minimal.
//!
//! Sert à fabriquer de petits objets sans compilateur : les tests de l'éditeur de liens, les
//! objets générés par nwasabi lui-même (stubs d'appels système, données externes). Le résultat
//! est un `.o` valide que `ElfObject::parse` relit et que les outils GNU savent lire.

use super::{
    Relocation, SHT_NOBITS, SHT_PROGBITS, SHT_REL, SHT_STRTAB, SHT_SYMTAB, SymbolBind, SymbolKind,
    SymbolSection,
};

/// Identifiant d'une section ajoutée à un [`ObjectWriter`] (ordre d'ajout, à partir de 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionId(pub usize);

/// Identifiant d'un symbole ajouté à un [`ObjectWriter`] (ordre d'ajout, à partir de 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolId(pub usize);

struct SectionSpec {
    name: String,
    kind: u32,
    flags: u32,
    align: u32,
    data: Vec<u8>,
    /// Taille pour une section sans contenu (`SHT_NOBITS`).
    nobits_size: u32,
    link: Option<SectionId>,
    relocations: Vec<Relocation>,
}

struct SymbolSpec {
    name: String,
    value: u32,
    size: u32,
    bind: SymbolBind,
    kind: SymbolKind,
    /// Pour `SymbolSection::Section(i)`, `i` est un [`SectionId`].
    section: SymbolSection,
}

/// Construit un objet ELF section par section.
#[derive(Default)]
pub struct ObjectWriter {
    sections: Vec<SectionSpec>,
    symbols: Vec<SymbolSpec>,
}

impl ObjectWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute une section avec contenu (`SHT_PROGBITS`).
    pub fn add_section(&mut self, name: &str, flags: u32, align: u32, data: &[u8]) -> SectionId {
        self.add_section_of_kind(name, SHT_PROGBITS, flags, align, data)
    }

    /// Ajoute une section avec contenu et un type précis (ex. `SHT_ARM_EXIDX`).
    pub fn add_section_of_kind(
        &mut self,
        name: &str,
        kind: u32,
        flags: u32,
        align: u32,
        data: &[u8],
    ) -> SectionId {
        self.sections.push(SectionSpec {
            name: name.to_owned(),
            kind,
            flags,
            align,
            data: data.to_vec(),
            nobits_size: 0,
            link: None,
            relocations: Vec::new(),
        });
        SectionId(self.sections.len() - 1)
    }

    /// Ajoute une section sans contenu dans le fichier (comme `.bss`).
    pub fn add_nobits(&mut self, name: &str, flags: u32, align: u32, size: u32) -> SectionId {
        let id = self.add_section_of_kind(name, SHT_NOBITS, flags, align, &[]);
        self.sections[id.0].nobits_size = size;
        id
    }

    /// Indique que `section` dépend de `target` (champ `sh_link`, ex. `.ARM.exidx` → son code).
    pub fn set_link(&mut self, section: SectionId, target: SectionId) {
        self.sections[section.0].link = Some(target);
    }

    pub fn add_symbol(
        &mut self,
        name: &str,
        value: u32,
        size: u32,
        bind: SymbolBind,
        kind: SymbolKind,
        section: SymbolSection,
    ) -> SymbolId {
        self.symbols.push(SymbolSpec {
            name: name.to_owned(),
            value,
            size,
            bind,
            kind,
            section,
        });
        SymbolId(self.symbols.len() - 1)
    }

    /// Ajoute une relocation à la section `section`.
    pub fn add_relocation(&mut self, section: SectionId, offset: u32, symbol: SymbolId, kind: u8) {
        // `symbol` est temporairement un SymbolId ; il sera converti en indice ELF dans `finish`.
        self.sections[section.0].relocations.push(Relocation {
            offset,
            symbol: symbol.0 as u32,
            kind,
        });
    }

    /// Produit les octets du fichier ELF.
    pub fn finish(self) -> Vec<u8> {
        // Indices ELF des symboles : le symbole nul d'abord, puis les locaux, puis les autres.
        let mut order: Vec<usize> = (0..self.symbols.len())
            .filter(|&i| self.symbols[i].bind == SymbolBind::Local)
            .collect();
        let local_count = order.len() + 1;
        order
            .extend((0..self.symbols.len()).filter(|&i| self.symbols[i].bind != SymbolBind::Local));
        let mut elf_index = vec![0u32; self.symbols.len()];
        for (rank, &i) in order.iter().enumerate() {
            elf_index[i] = rank as u32 + 1;
        }

        let user_count = self.sections.len();
        let rel_sections: Vec<usize> = (0..user_count)
            .filter(|&i| !self.sections[i].relocations.is_empty())
            .collect();
        let symtab_index = 1 + user_count + rel_sections.len();
        let strtab_index = symtab_index + 1;
        let shstrtab_index = symtab_index + 2;

        let mut shstr = StringTable::new();
        let mut out = vec![0u8; 52];
        let mut headers: Vec<[u32; 10]> = vec![[0; 10]; shstrtab_index + 1];

        for (i, s) in self.sections.iter().enumerate() {
            pad(&mut out, 4);
            let offset = out.len() as u32;
            out.extend_from_slice(&s.data);
            let size = if s.kind == SHT_NOBITS {
                s.nobits_size
            } else {
                s.data.len() as u32
            };
            let link = s.link.map_or(0, |l| l.0 as u32 + 1);
            headers[i + 1] = [
                shstr.add(&s.name),
                s.kind,
                s.flags,
                0,
                offset,
                size,
                link,
                0,
                s.align,
                0,
            ];
        }
        for (k, &i) in rel_sections.iter().enumerate() {
            pad(&mut out, 4);
            let offset = out.len() as u32;
            for r in &self.sections[i].relocations {
                out.extend_from_slice(&r.offset.to_le_bytes());
                let sym = elf_index[r.symbol as usize];
                out.extend_from_slice(&((sym << 8) | u32::from(r.kind)).to_le_bytes());
            }
            let size = out.len() as u32 - offset;
            let name = format!(".rel{}", self.sections[i].name);
            headers[1 + user_count + k] = [
                shstr.add(&name),
                SHT_REL,
                0x40, // SHF_INFO_LINK
                0,
                offset,
                size,
                symtab_index as u32,
                i as u32 + 1,
                4,
                8,
            ];
        }

        let mut strtab = StringTable::new();
        pad(&mut out, 4);
        let symtab_offset = out.len() as u32;
        out.extend_from_slice(&[0u8; 16]);
        for &i in &order {
            let s = &self.symbols[i];
            out.extend_from_slice(&strtab.add(&s.name).to_le_bytes());
            out.extend_from_slice(&s.value.to_le_bytes());
            out.extend_from_slice(&s.size.to_le_bytes());
            out.push((bind_code(s.bind) << 4) | kind_code(s.kind));
            out.push(0);
            let shndx: u16 = match s.section {
                SymbolSection::Undefined => 0,
                SymbolSection::Absolute => 0xfff1,
                SymbolSection::Common => 0xfff2,
                SymbolSection::Section(id) => id as u16 + 1,
            };
            out.extend_from_slice(&shndx.to_le_bytes());
        }
        let symtab_size = out.len() as u32 - symtab_offset;
        headers[symtab_index] = [
            shstr.add(".symtab"),
            SHT_SYMTAB,
            0,
            0,
            symtab_offset,
            symtab_size,
            strtab_index as u32,
            local_count as u32,
            4,
            16,
        ];

        let strtab_offset = out.len() as u32;
        out.extend_from_slice(&strtab.bytes);
        headers[strtab_index] = [
            shstr.add(".strtab"),
            SHT_STRTAB,
            0,
            0,
            strtab_offset,
            strtab.bytes.len() as u32,
            0,
            0,
            1,
            0,
        ];

        let shstr_name = shstr.add(".shstrtab");
        let shstr_offset = out.len() as u32;
        out.extend_from_slice(&shstr.bytes);
        headers[shstrtab_index] = [
            shstr_name,
            SHT_STRTAB,
            0,
            0,
            shstr_offset,
            shstr.bytes.len() as u32,
            0,
            0,
            1,
            0,
        ];

        pad(&mut out, 4);
        let section_table_offset = out.len() as u32;
        for h in &headers {
            for word in h {
                out.extend_from_slice(&word.to_le_bytes());
            }
        }

        // En-tête ELF : ELF32, little-endian, ET_REL, EM_ARM, EABI v5.
        out[..16].copy_from_slice(b"\x7fELF\x01\x01\x01\0\0\0\0\0\0\0\0\0");
        out[16..18].copy_from_slice(&1u16.to_le_bytes());
        out[18..20].copy_from_slice(&40u16.to_le_bytes());
        out[20..24].copy_from_slice(&1u32.to_le_bytes());
        out[32..36].copy_from_slice(&section_table_offset.to_le_bytes());
        out[36..40].copy_from_slice(&0x0500_0000u32.to_le_bytes());
        out[40..42].copy_from_slice(&52u16.to_le_bytes());
        out[46..48].copy_from_slice(&40u16.to_le_bytes());
        out[48..50].copy_from_slice(&(headers.len() as u16).to_le_bytes());
        out[50..52].copy_from_slice(&(shstrtab_index as u16).to_le_bytes());
        out
    }
}

fn pad(out: &mut Vec<u8>, align: usize) {
    while out.len() % align != 0 {
        out.push(0);
    }
}

fn bind_code(bind: SymbolBind) -> u8 {
    match bind {
        SymbolBind::Local => 0,
        SymbolBind::Global => 1,
        SymbolBind::Weak => 2,
        SymbolBind::Other(n) => n,
    }
}

fn kind_code(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::NoType => 0,
        SymbolKind::Object => 1,
        SymbolKind::Func => 2,
        SymbolKind::Section => 3,
        SymbolKind::File => 4,
        SymbolKind::Common => 5,
        SymbolKind::Tls => 6,
        SymbolKind::Other(n) => n,
    }
}

/// Table de chaînes ELF : chaînes NUL-terminées, la première (offset 0) est la chaîne vide.
struct StringTable {
    bytes: Vec<u8>,
}

impl StringTable {
    fn new() -> Self {
        Self { bytes: vec![0] }
    }

    fn add(&mut self, s: &str) -> u32 {
        if s.is_empty() {
            return 0;
        }
        let offset = self.bytes.len() as u32;
        self.bytes.extend_from_slice(s.as_bytes());
        self.bytes.push(0);
        offset
    }
}
