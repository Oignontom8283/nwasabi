//! En-tête d'un fichier ELF 32 bits little-endian.

use super::error::ElfError;
use super::reader::Reader;

const EI_CLASS_32: u8 = 1;
const EI_DATA_LE: u8 = 1;
const ET_REL: u16 = 1;
const EM_ARM: u16 = 40;

/// Les champs de l'en-tête dont l'éditeur de liens a besoin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// `e_flags` : pour ARM, l'octet de poids fort donne la version d'ABI (5 = EABI v5).
    pub flags: u32,
    pub(crate) section_table_offset: usize,
    pub(crate) section_entry_size: usize,
    pub(crate) section_count: usize,
    pub(crate) section_names_index: usize,
}

impl Header {
    /// Lit et valide l'en-tête : objet relogeable ARM, 32 bits, little-endian.
    pub(crate) fn parse(reader: &Reader<'_>) -> Result<Self, ElfError> {
        let ident = reader.slice(0, 16, "identification ELF")?;
        if &ident[..4] != b"\x7fELF" {
            return Err(ElfError::NotElf);
        }
        if ident[4] != EI_CLASS_32 {
            return Err(ElfError::UnsupportedClass(ident[4]));
        }
        if ident[5] != EI_DATA_LE {
            return Err(ElfError::UnsupportedEndianness(ident[5]));
        }
        let kind = reader.u16_at(16, "e_type")?;
        if kind != ET_REL {
            return Err(ElfError::UnsupportedType(kind));
        }
        let machine = reader.u16_at(18, "e_machine")?;
        if machine != EM_ARM {
            return Err(ElfError::UnsupportedMachine(machine));
        }
        let section_count = usize::from(reader.u16_at(48, "e_shnum")?);
        if section_count == 0 {
            return Err(ElfError::NoSections);
        }
        Ok(Self {
            flags: reader.u32_at(36, "e_flags")?,
            section_table_offset: reader.u32_at(32, "e_shoff")? as usize,
            section_entry_size: usize::from(reader.u16_at(46, "e_shentsize")?),
            section_count,
            section_names_index: usize::from(reader.u16_at(50, "e_shstrndx")?),
        })
    }

    /// Version d'ABI ARM annoncée par `e_flags` (5 pour EABI v5).
    pub fn abi_version(&self) -> u8 {
        (self.flags >> 24) as u8
    }
}
