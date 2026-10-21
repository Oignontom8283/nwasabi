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

// === 1. Résolution des symboles ===

/// Comment un symbole utilisé par une relocation se résout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resolved {
    /// Défini dans un objet (celui qui l'utilise ou un autre).
    Def(SymRef),
    /// Défini par l'éditeur de liens (adresses de `.data`, du tas…).
    Linker,
    /// Faible et défini nulle part : vaut 0.
    UndefinedWeak,
    /// Défini nulle part : erreur si quelqu'un l'utilise vraiment.
    Undefined,
}

/// Force d'une définition : un symbole fort l'emporte sur un faible, qui l'emporte sur un commun.
fn rank(sym: &Symbol) -> u8 {
    match (sym.section, sym.bind) {
        (SymbolSection::Common, _) => 0,
        (_, SymbolBind::Weak) => 1,
        _ => 2,
    }
}

/// Table des définitions globales. À force égale, la première définition gagne
/// (équivalent de `--allow-multiple-definition`).
fn build_globals<'a>(objects: &'a [Obj<'_>]) -> HashMap<&'a str, SymRef> {
    let mut globals: HashMap<&str, SymRef> = HashMap::new();
    for (o, obj) in objects.iter().enumerate() {
        for (i, sym) in obj.elf.symbols.iter().enumerate().skip(1) {
            let global = matches!(sym.bind, SymbolBind::Global | SymbolBind::Weak);
            if !global || !sym.is_defined() || sym.name.is_empty() {
                continue;
            }
            if matches!(sym.kind, SymbolKind::Section | SymbolKind::File) {
                continue;
            }
            let new = SymRef { obj: o, index: i };
            match globals.get(sym.name.as_str()) {
                None => {
                    globals.insert(&sym.name, new);
                }
                Some(old) => {
                    let old_sym = &objects[old.obj].elf.symbols[old.index];
                    let better = rank(sym) > rank(old_sym)
                        || (rank(sym) == 0 && rank(old_sym) == 0 && sym.size > old_sym.size);
                    if better {
                        globals.insert(&sym.name, new);
                    }
                }
            }
        }
    }
    globals
}

fn resolve(objects: &[Obj<'_>], globals: &HashMap<&str, SymRef>, r: SymRef) -> Resolved {
    let sym = &objects[r.obj].elf.symbols[r.index];
    if sym.bind == SymbolBind::Local {
        return Resolved::Def(r);
    }
    if let Some(def) = globals.get(sym.name.as_str()) {
        return Resolved::Def(*def);
    }
    if LINKER_SYMBOLS.contains(&sym.name.as_str()) {
        return Resolved::Linker;
    }
    if sym.is_weak() {
        Resolved::UndefinedWeak
    } else {
        Resolved::Undefined
    }
}

