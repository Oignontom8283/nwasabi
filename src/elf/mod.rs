//! Lecture des fichiers ELF 32 bits ARM relogeables (`.o`).
//!
//! Un fichier `.nwa` est un tel fichier : ce n'est pas un programme prêt à être exécuté, il doit
//! encore être « lié » (voir le module `link`). Ce module se limite à lire le fichier de façon
//! sûre : toutes les positions et tailles sont vérifiées, jamais de `panic` sur une entrée
//! malformée.

mod error;
mod header;
mod reader;
mod reloc;
mod section;
mod symbol;
mod writer;

pub use error::ElfError;
pub use header::Header;
pub use reloc::*;
pub use section::*;
pub use symbol::{Symbol, SymbolBind, SymbolKind, SymbolSection};
pub use writer::{ObjectWriter, SectionId, SymbolId};

use reader::Reader;

/// Bornes protégeant contre les fichiers démesurés (utile quand le fichier vient d'un inconnu,
/// par exemple déposé dans une page web).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_file_size: usize,
    pub max_sections: usize,
    pub max_symbols: usize,
    pub max_relocations: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_size: 64 << 20,
            max_sections: 100_000,
            max_symbols: 2_000_000,
            max_relocations: 8_000_000,
        }
    }
}

const SYMBOL_SIZE: usize = 16;
const SECTION_HEADER_SIZE: usize = 40;
const REL_SIZE: usize = 8;
const SHN_ABS: u16 = 0xfff1;
const SHN_COMMON: u16 = 0xfff2;
const SHN_LORESERVE: u16 = 0xff00;

/// Un fichier ELF relogeable lu en mémoire. Emprunte les octets du fichier.
#[derive(Debug)]
pub struct ElfObject<'a> {
    data: &'a [u8],
    pub header: Header,
    pub sections: Vec<Section>,
    /// Table des symboles (l'entrée 0 est le symbole nul).
    pub symbols: Vec<Symbol>,
    /// Relocations, rangées par indice de la section à patcher.
    relocations: Vec<Vec<Relocation>>,
}

impl<'a> ElfObject<'a> {
    /// Lit un fichier ELF relogeable ARM.
    pub fn parse(data: &'a [u8], limits: &Limits) -> Result<Self, ElfError> {
        if data.len() > limits.max_file_size {
            return Err(ElfError::TooMany {
                what: "octets",
                count: data.len(),
                limit: limits.max_file_size,
            });
        }
        let reader = Reader::new(data);
        let header = Header::parse(&reader)?;
        let sections = parse_sections(&reader, &header, limits)?;
        let symbols = parse_symbols(&reader, &sections, limits)?;
        let relocations = parse_relocations(&reader, &sections, symbols.len(), limits)?;
        Ok(Self {
            data,
            header,
            sections,
            symbols,
            relocations,
        })
    }

    /// Contenu d'une section (vide pour une section sans contenu comme `.bss`).
    pub fn section_data(&self, index: usize) -> &'a [u8] {
        let section = &self.sections[index];
        if section.is_nobits() {
            return &[];
        }
        // Les bornes ont été vérifiées pendant `parse`.
        &self.data[section.offset..section.offset + section.size as usize]
    }

    /// Relocations à appliquer à la section d'indice donné.
    pub fn relocations(&self, section_index: usize) -> &[Relocation] {
        &self.relocations[section_index]
    }

    /// Cherche une section par son nom exact.
    pub fn section_by_name(&self, name: &str) -> Option<usize> {
        self.sections.iter().position(|s| s.name == name)
    }
}

fn cstr_lossy(table: &[u8], offset: usize, what: &'static str) -> Result<String, ElfError> {
    let tail = table.get(offset..).ok_or(ElfError::InvalidString(what))?;
    let end = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or(ElfError::InvalidString(what))?;
    Ok(String::from_utf8_lossy(&tail[..end]).into_owned())
}

fn parse_sections(
    reader: &Reader<'_>,
    header: &Header,
    limits: &Limits,
) -> Result<Vec<Section>, ElfError> {
    if header.section_count > limits.max_sections {
        return Err(ElfError::TooMany {
            what: "sections",
            count: header.section_count,
            limit: limits.max_sections,
        });
    }
    if header.section_entry_size < SECTION_HEADER_SIZE {
        return Err(ElfError::Truncated("en-tête de section"));
    }
    let table_bytes = header
        .section_count
        .checked_mul(header.section_entry_size)
        .ok_or(ElfError::Truncated("table des sections"))?;
    reader.slice(
        header.section_table_offset,
        table_bytes,
        "table des sections",
    )?;

    // Premier passage : champs numériques (le nom dépend de la table des noms).
    struct Raw {
        name: usize,
        kind: u32,
        flags: u32,
        addr: u32,
        offset: usize,
        size: u32,
        link: u32,
        info: u32,
        align: u32,
        entsize: u32,
    }
    let mut raws = Vec::with_capacity(header.section_count);
    for i in 0..header.section_count {
        let at = header.section_table_offset + i * header.section_entry_size;
        let raw = Raw {
            name: reader.u32_at(at, "nom de section")? as usize,
            kind: reader.u32_at(at + 4, "type de section")?,
            flags: reader.u32_at(at + 8, "drapeaux de section")?,
            addr: reader.u32_at(at + 12, "adresse de section")?,
            offset: reader.u32_at(at + 16, "position de section")? as usize,
            size: reader.u32_at(at + 20, "taille de section")?,
            link: reader.u32_at(at + 24, "lien de section")?,
            info: reader.u32_at(at + 28, "info de section")?,
            align: reader.u32_at(at + 32, "alignement de section")?,
            entsize: reader.u32_at(at + 36, "taille d'entrée de section")?,
        };
        if raw.kind != SHT_NOBITS && raw.kind != 0 {
            let end = raw.offset.checked_add(raw.size as usize);
            if end.is_none_or(|e| e > reader.len()) {
                return Err(ElfError::SectionOutOfBounds { index: i });
            }
        }
        raws.push(raw);
    }

    let names = raws
        .get(header.section_names_index)
        .ok_or(ElfError::InvalidString(
            "table des noms de sections absente",
        ))?;
    let names_table = reader.slice(
        names.offset,
        names.size as usize,
        "table des noms de sections",
    )?;

    raws.iter()
        .map(|r| {
            Ok(Section {
                name: cstr_lossy(names_table, r.name, "nom de section")?,
                kind: r.kind,
                flags: r.flags,
                addr: r.addr,
                offset: r.offset,
                size: r.size,
                link: r.link,
                info: r.info,
                align: r.align,
                entsize: r.entsize,
            })
        })
        .collect()
}

fn parse_symbols(
    reader: &Reader<'_>,
    sections: &[Section],
    limits: &Limits,
) -> Result<Vec<Symbol>, ElfError> {
    let Some(symtab_index) = sections.iter().position(|s| s.kind == SHT_SYMTAB) else {
        return Ok(Vec::new());
    };
    let symtab = &sections[symtab_index];
    let strtab_section = sections
        .get(symtab.link as usize)
        .filter(|s| s.kind == SHT_STRTAB)
        .ok_or(ElfError::Malformed {
            section: symtab_index,
            what: "table de chaînes absente",
        })?;
    let strtab = reader.slice(
        strtab_section.offset,
        strtab_section.size as usize,
        "chaînes de symboles",
    )?;

    let count = symtab.size as usize / SYMBOL_SIZE;
    if count > limits.max_symbols {
        return Err(ElfError::TooMany {
            what: "symboles",
            count,
            limit: limits.max_symbols,
        });
    }
    let mut symbols = Vec::with_capacity(count);
    for i in 0..count {
        let at = symtab.offset + i * SYMBOL_SIZE;
        let name_offset = reader.u32_at(at, "nom de symbole")? as usize;
        let value = reader.u32_at(at + 4, "valeur de symbole")?;
        let size = reader.u32_at(at + 8, "taille de symbole")?;
        let info = reader.u8_at(at + 12, "info de symbole")?;
        let shndx = reader.u16_at(at + 14, "section de symbole")?;
        let section = match shndx {
            0 => SymbolSection::Undefined,
            SHN_ABS => SymbolSection::Absolute,
            SHN_COMMON => SymbolSection::Common,
            n if n >= SHN_LORESERVE => {
                return Err(ElfError::UnsupportedSymbolSection {
                    symbol: i,
                    index: n,
                });
            }
            n if usize::from(n) < sections.len() => SymbolSection::Section(usize::from(n)),
            _ => {
                return Err(ElfError::SectionOutOfBounds {
                    index: usize::from(shndx),
                });
            }
        };
        symbols.push(Symbol {
            name: cstr_lossy(strtab, name_offset, "nom de symbole")?,
            value,
            size,
            bind: SymbolBind::from_info(info),
            kind: SymbolKind::from_info(info),
            section,
        });
    }
    Ok(symbols)
}

fn parse_relocations(
    reader: &Reader<'_>,
    sections: &[Section],
    symbol_count: usize,
    limits: &Limits,
) -> Result<Vec<Vec<Relocation>>, ElfError> {
    let mut relocations: Vec<Vec<Relocation>> = vec![Vec::new(); sections.len()];
    let mut total = 0usize;
    for (index, section) in sections.iter().enumerate() {
        if section.kind == SHT_RELA {
            return Err(ElfError::RelaNotSupported { section: index });
        }
        if section.kind != SHT_REL {
            continue;
        }
        let target = section.info as usize;
        if target >= sections.len() {
            return Err(ElfError::Malformed {
                section: index,
                what: "section cible inconnue",
            });
        }
        let count = section.size as usize / REL_SIZE;
        total = total.saturating_add(count);
        if total > limits.max_relocations {
            return Err(ElfError::TooMany {
                what: "relocations",
                count: total,
                limit: limits.max_relocations,
            });
        }
        let list = &mut relocations[target];
        list.reserve(count);
        for i in 0..count {
            let at = section.offset + i * REL_SIZE;
            let offset = reader.u32_at(at, "offset de relocation")?;
            let info = reader.u32_at(at + 4, "info de relocation")?;
            let symbol = info >> 8;
            if symbol as usize >= symbol_count {
                return Err(ElfError::Malformed {
                    section: index,
                    what: "symbole de relocation inconnu",
                });
            }
            list.push(Relocation {
                offset,
                symbol,
                kind: (info & 0xff) as u8,
            });
        }
    }
    Ok(relocations)
}

#[cfg(test)]
mod tests;
