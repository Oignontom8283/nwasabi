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

// === 2. Suppression des sections inutiles ===

/// Résultat de la recherche des sections utiles.
struct Reachability {
    /// `live[o][s]` vaut vrai pour les sections à garder.
    live: Vec<Vec<bool>>,
    /// Variables communes (`COMMON`) réellement utilisées, par nom.
    commons: HashSet<String>,
}

struct Context<'a, 'b> {
    objects: &'a [Obj<'b>],
    globals: HashMap<&'a str, SymRef>,
}

impl Context<'_, '_> {
    fn resolve(&self, r: SymRef) -> Resolved {
        resolve(self.objects, &self.globals, r)
    }

    /// Marque `start` et tout ce qu'elle référence.
    fn mark(&self, reach: &mut Reachability, start: SecId) {
        let mut pending = vec![start];
        while let Some((o, s)) = pending.pop() {
            if reach.live[o][s] || !self.objects[o].elf.sections[s].is_alloc() {
                continue;
            }
            reach.live[o][s] = true;
            // Les relocations « NONE » ne changent rien au contenu mais comptent comme des
            // dépendances (c'est ainsi qu'une table d'exceptions retient sa routine).
            for reloc in self.objects[o].elf.relocations(s) {
                let target = SymRef {
                    obj: o,
                    index: reloc.symbol as usize,
                };
                if let Resolved::Def(def) = self.resolve(target) {
                    let sym = &self.objects[def.obj].elf.symbols[def.index];
                    match sym.section {
                        SymbolSection::Section(next) => pending.push((def.obj, next)),
                        SymbolSection::Common => {
                            reach.commons.insert(sym.name.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn reachability(&self, roots: &[SecId]) -> Reachability {
        let mut reach = Reachability {
            live: self
                .objects
                .iter()
                .map(|o| vec![false; o.elf.sections.len()])
                .collect(),
            commons: HashSet::new(),
        };
        for &root in roots {
            self.mark(&mut reach, root);
        }
        // Les tables de déroulement (`.ARM.exidx`) sont gardées avec le code qu'elles décrivent :
        // elles désignent des routines (`__aeabi_unwind_cpp_pr0`…) que l'éditeur de liens officiel
        // conserve donc aussi. Les tables elles-mêmes ne sont pas placées (voir `classify`).
        loop {
            let mut changed = false;
            for (o, obj) in self.objects.iter().enumerate() {
                for (s, section) in obj.elf.sections.iter().enumerate() {
                    if section.kind == SHT_ARM_EXIDX
                        && !reach.live[o][s]
                        && reach.live[o]
                            .get(section.link as usize)
                            .copied()
                            .unwrap_or(false)
                    {
                        self.mark(&mut reach, (o, s));
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        reach
    }
}

// === 3. Placement en flash et en RAM ===

/// Où une section de l'image va.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Class {
    ApiLevel,
    Name,
    Icon,
    Text,
    Rodata,
    Data,
    Bss,
}

impl Class {
    fn public(self) -> SectionClass {
        match self {
            Self::ApiLevel | Self::Name | Self::Icon => SectionClass::Header,
            Self::Text => SectionClass::Text,
            Self::Rodata => SectionClass::Rodata,
            Self::Data => SectionClass::Data,
            Self::Bss => SectionClass::Bss,
        }
    }
}

/// Famille d'une section d'après son nom, comme le ferait un script de link :
/// `.text` et `.text.*` vont dans le code, `.rodata*` dans les constantes, etc.
fn named_class(name: &str) -> Option<(Class, u8)> {
    for (class, base) in [
        (Class::Text, ".text"),
        (Class::Rodata, ".rodata"),
        (Class::Data, ".data"),
        (Class::Bss, ".bss"),
    ] {
        if name == base {
            return Some((class, 0));
        }
        if name
            .strip_prefix(base)
            .is_some_and(|rest| rest.starts_with('.'))
        {
            return Some((class, 1));
        }
    }
    None
}

/// Classe et rang d'une section ; `None` pour une section qui n'occupe pas de mémoire.
///
/// Le rang reproduit l'ordre du script de link : d'abord les sections nommées exactement
/// `.text`, puis `.text.*`, puis les autres sections de même nature (« orphelines »).
fn classify(section: &Section) -> Result<Option<(Class, u8)>, LinkError> {
    if !section.is_alloc() {
        return Ok(None);
    }
    if section.flags & SHF_TLS != 0 {
        return Err(LinkError::Unsupported("sections locales aux threads (TLS)"));
    }
    let special = match section.name.as_str() {
        API_LEVEL_SECTION => Some(Class::ApiLevel),
        NAME_SECTION => Some(Class::Name),
        ICON_SECTION => Some(Class::Icon),
        _ => None,
    };
    if let Some(class) = special {
        return Ok(Some((class, 0)));
    }
    // Tables de déroulement de la pile (exceptions C++) : l'éditeur de liens officiel les construit
    // mais personne ne les lit sur Epsilon ; elles ne sont pas placées.
    if section.kind == SHT_ARM_EXIDX || section.name.starts_with(".ARM.extab") {
        return Ok(None);
    }
    if let Some(found) = named_class(&section.name) {
        return Ok(Some(found));
    }
    let class = if section.is_executable() {
        Class::Text
    } else if section.is_nobits() {
        Class::Bss
    } else if section.is_writable() {
        Class::Data
    } else {
        Class::Rodata
    };
    Ok(Some((class, 2)))
}

