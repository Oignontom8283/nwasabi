//! Cœur de l'édition de liens : symboles, suppression des sections inutiles, placement,
//! relocations et assemblage de l'image.

use std::collections::{HashMap, HashSet};

use super::relocate::{self, Target};
use super::runtime::linker_symbols as ls;
use super::{
    APP_SECTOR_SIZE, DataSegment, LayoutParams, LinkError, LinkWarning, LinkedApp, LinkedSymbol,
    Origin, PlacedSection, Region, SectionClass,
};
use crate::elf::{
    ElfObject, R_ARM_NONE, SHF_TLS, SHT_ARM_EXIDX, Section, Symbol, SymbolBind, SymbolKind,
    SymbolSection,
};

/// Un objet ELF en entrée de l'édition de liens.
pub(super) struct Obj<'a> {
    pub origin: Origin,
    pub elf: ElfObject<'a>,
}

/// (indice de l'objet, indice de la section)
type SecId = (usize, usize);

/// (indice de l'objet, indice du symbole)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SymRef {
    obj: usize,
    index: usize,
}

const MAGIC: u32 = 0xdec0_beba;
const HEADER_SIZE: u32 = 32;
const API_LEVEL_SECTION: &str = ".rodata.eadk_api_level";
const NAME_SECTION: &str = ".rodata.eadk_app_name";
const ICON_SECTION: &str = ".rodata.eadk_app_icon";

/// Symboles que le « script de link » définit lui-même.
const LINKER_SYMBOLS: [&str; 9] = [
    ls::TRAMPOLINE,
    ls::HEAP_START,
    ls::HEAP_END,
    ls::DATA_START_FLASH,
    ls::DATA_START_RAM,
    ls::DATA_END_RAM,
    ls::BSS_START_RAM,
    ls::BSS_END_RAM,
    ls::APP_END,
];

