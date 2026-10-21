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

fn align_up(value: u64, alignment: u32) -> u64 {
    let a = u64::from(alignment.max(1));
    value.div_ceil(a) * a
}

#[derive(Debug, Clone, Copy)]
struct Placement {
    /// Adresse d'exécution.
    vma: u32,
    /// Adresse de la copie en flash (absente pour `.bss`).
    flash: Option<u32>,
    class: Class,
}

/// Résultat du placement.
struct Layout {
    placements: HashMap<SecId, Placement>,
    /// Valeurs des symboles définis par l'éditeur de liens.
    symbols: HashMap<&'static str, u32>,
    /// Adresse des variables communes.
    commons: HashMap<String, u32>,
    sections: Vec<PlacedSection>,
    /// Longueur de l'image en flash, en octets depuis `flash_start`.
    image_len: u32,
    app_end: u32,
    data: DataSegment,
    bss: Region,
    heap: Region,
}

struct Placer<'a, 'b> {
    objects: &'a [Obj<'b>],
}

impl Placer<'_, '_> {
    fn section(&self, (o, s): SecId) -> &Section {
        &self.objects[o].elf.sections[s]
    }

    /// Place des sections à la suite, à partir de `cursor`. L'ensemble démarre à l'alignement le
    /// plus exigeant de ses membres (au moins `min_align`) ; chaque section respecte le sien.
    /// Renvoie l'adresse de chacune, en laissant `cursor` à la fin ; ne fait rien s'il n'y en a pas.
    fn place(&self, cursor: &mut u64, ids: &[SecId], min_align: u32) -> Vec<u64> {
        if ids.is_empty() {
            return Vec::new();
        }
        let group_align = ids
            .iter()
            .map(|&id| self.section(id).alignment())
            .fold(min_align, u32::max);
        let mut at = align_up(*cursor, group_align);
        let mut addresses = Vec::with_capacity(ids.len());
        for &id in ids {
            let section = self.section(id);
            at = align_up(at, section.alignment());
            addresses.push(at);
            at += u64::from(section.size);
        }
        *cursor = at;
        addresses
    }

    fn group_align(&self, ids: &[SecId], min_align: u32) -> u32 {
        ids.iter()
            .map(|&id| self.section(id).alignment())
            .fold(min_align, u32::max)
    }
}

fn to_u32(value: u64) -> u32 {
    u32::try_from(value).expect("adresse vérifiée contre la taille des zones")
}

fn layout(
    objects: &[Obj<'_>],
    reach: &Reachability,
    params: &LayoutParams,
) -> Result<Layout, LinkError> {
    // Sections vivantes, classées puis triées dans l'ordre du script de link.
    let mut groups: HashMap<Class, Vec<(u8, SecId)>> = HashMap::new();
    for (o, obj) in objects.iter().enumerate() {
        for (s, section) in obj.elf.sections.iter().enumerate() {
            if reach.live[o][s]
                && let Some((class, rank)) = classify(section)?
            {
                groups.entry(class).or_default().push((rank, (o, s)));
            }
        }
    }
    let mut ids_of = |class: Class| -> Vec<SecId> {
        let mut list = groups.remove(&class).unwrap_or_default();
        list.sort();
        list.into_iter().map(|(_, id)| id).collect()
    };
    let api = ids_of(Class::ApiLevel);
    let names = ids_of(Class::Name);
    let icons = ids_of(Class::Icon);
    let text = ids_of(Class::Text);
    let rodata = ids_of(Class::Rodata);
    let data = ids_of(Class::Data);
    let bss = ids_of(Class::Bss);

    let placer = Placer { objects };
    let flash_start = u64::from(params.flash_start);
    let mut placements: HashMap<SecId, Placement> = HashMap::new();
    let mut sections: Vec<PlacedSection> = Vec::new();
    let mut flash_cursor = flash_start;
    let mut image_end = flash_start;

    let record = |placements: &mut HashMap<SecId, Placement>,
                  sections: &mut Vec<PlacedSection>,
                  id: SecId,
                  class: Class,
                  vma: u64,
                  flash: Option<u64>| {
        let section = &objects[id.0].elf.sections[id.1];
        placements.insert(
            id,
            Placement {
                vma: to_u32(vma),
                flash: flash.map(to_u32),
                class,
            },
        );
        sections.push(PlacedSection {
            name: section.name.clone(),
            origin: objects[id.0].origin,
            class: class.public(),
            address: to_u32(vma),
            flash_address: flash.map(to_u32),
            size: section.size,
            alignment: section.alignment(),
        });
    };

    // En-tête d'application : 32 octets, dont le niveau d'API (4 octets) juste après le magic.
    let [api_id] = api[..] else {
        return Err(match api.first() {
            None => LinkError::MissingSection(API_LEVEL_SECTION),
            Some(&id) => LinkError::InvalidApiLevel {
                size: placer.section(id).size,
            },
        });
    };
    if placer.section(api_id).size != 4 {
        return Err(LinkError::InvalidApiLevel {
            size: placer.section(api_id).size,
        });
    }
    if names.is_empty() {
        return Err(LinkError::MissingSection(NAME_SECTION));
    }
    if icons.is_empty() {
        return Err(LinkError::MissingSection(ICON_SECTION));
    }
    record(
        &mut placements,
        &mut sections,
        api_id,
        Class::ApiLevel,
        flash_start + 4,
        Some(flash_start + 4),
    );
    flash_cursor += u64::from(HEADER_SIZE);

    for (class, ids) in [(Class::Name, &names), (Class::Icon, &icons)] {
        let addresses = placer.place(&mut flash_cursor, ids, 1);
        for (&id, &at) in ids.iter().zip(&addresses) {
            record(&mut placements, &mut sections, id, class, at, Some(at));
        }
    }
    // Code puis constantes : tout est chargé depuis la flash.
    let mut app_end = flash_cursor;
    for (class, ids) in [(Class::Text, &text), (Class::Rodata, &rodata)] {
        let addresses = placer.place(&mut flash_cursor, ids, 1);
        for (&id, &at) in ids.iter().zip(&addresses) {
            record(&mut placements, &mut sections, id, class, at, Some(at));
        }
        if class == Class::Rodata {
            app_end = flash_cursor;
        }
    }
    image_end = image_end.max(flash_cursor);

    // .data : exécuté en RAM, copie initiale en flash juste après le reste.
    let data_align = placer.group_align(&data, 4);
    // La copie initiale suit directement le reste, sans alignement (comme `AT>FLASH` du script de
    // link) : `_start` la copie octet par octet.
    let data_flash = flash_cursor;
    let data_ram = align_up(u64::from(params.ram_start), data_align);
    let mut ram_cursor = data_ram;
    let addresses = placer.place(&mut ram_cursor, &data, 4);
    for (&id, &at) in data.iter().zip(&addresses) {
        record(
            &mut placements,
            &mut sections,
            id,
            Class::Data,
            at,
            Some(data_flash + (at - data_ram)),
        );
    }
    let data_end = ram_cursor;
    let data_size = data_end - data_ram;
    if data_size > 0 {
        image_end = image_end.max(data_flash + data_size);
    }

    // .bss puis variables communes : en RAM seulement.
    let bss_align = placer.group_align(&bss, 4);
    let bss_start = align_up(data_end, bss_align);
    ram_cursor = bss_start;
    let addresses = placer.place(&mut ram_cursor, &bss, 4);
    for (&id, &at) in bss.iter().zip(&addresses) {
        record(&mut placements, &mut sections, id, Class::Bss, at, None);
    }
    // Le script de link ne prévoit pas `COMMON` : ces variables sont placées après `.bss`, donc hors
    // de la zone que `_start` met à zéro (comportement observé de l'outil officiel).
    let bss_end = ram_cursor;
    let mut commons: HashMap<String, u32> = HashMap::new();
    let mut common_defs: Vec<(&str, u32, u32, Origin)> = Vec::new(); // nom, taille, alignement, origine
    for obj in objects {
        for sym in &obj.elf.symbols {
            if sym.section == SymbolSection::Common && reach.commons.contains(&sym.name) {
                match common_defs.iter_mut().find(|c| c.0 == sym.name) {
                    Some(c) => {
                        c.1 = c.1.max(sym.size);
                        c.2 = c.2.max(sym.value);
                    }
                    None => common_defs.push((&sym.name, sym.size, sym.value, obj.origin)),
                }
            }
        }
    }
    for (name, size, alignment, origin) in common_defs {
        let at = align_up(ram_cursor, alignment);
        commons.insert(name.to_owned(), to_u32(at));
        sections.push(PlacedSection {
            name: name.to_owned(),
            origin,
            class: SectionClass::Common,
            address: to_u32(at),
            flash_address: None,
            size,
            alignment: alignment.max(1),
        });
        ram_cursor = at + u64::from(size);
    }
    let commons_end = ram_cursor;

    // Vérifications de capacité.
    if image_end > params.flash_end() {
        return Err(LinkError::FlashOverflow {
            needed: image_end - flash_start,
            available: params.flash_length,
        });
    }
    if commons_end > params.ram_end() {
        return Err(LinkError::RamOverflow {
            needed: commons_end - u64::from(params.ram_start),
            available: params.ram_length,
        });
    }
    let heap_start = commons_end;
    let heap_end = params.ram_end();

    let symbols = HashMap::from([
        (ls::TRAMPOLINE, params.trampoline_address),
        (ls::HEAP_START, to_u32(heap_start)),
        (ls::HEAP_END, to_u32(heap_end)),
        (ls::DATA_START_FLASH, to_u32(data_flash)),
        (ls::DATA_START_RAM, to_u32(data_ram)),
        (ls::DATA_END_RAM, to_u32(data_end)),
        (ls::BSS_START_RAM, to_u32(bss_start)),
        (ls::BSS_END_RAM, to_u32(bss_end)),
        (ls::APP_END, to_u32(app_end)),
    ]);
    Ok(Layout {
        placements,
        symbols,
        commons,
        sections,
        image_len: to_u32(image_end - flash_start),
        app_end: to_u32(app_end),
        data: DataSegment {
            address: to_u32(data_ram),
            flash_address: to_u32(data_flash),
            size: to_u32(data_size),
        },
        bss: Region {
            address: to_u32(bss_start),
            size: to_u32(bss_end - bss_start),
        },
        heap: Region {
            address: to_u32(heap_start),
            size: to_u32(heap_end - heap_start),
        },
    })
}

// === 4. Relocations et assemblage de l'image ===

/// Nom lisible d'un symbole pour un message d'erreur (un symbole de section n'a pas de nom).
fn display_name(objects: &[Obj<'_>], r: SymRef) -> String {
    let sym = &objects[r.obj].elf.symbols[r.index];
    match (sym.name.as_str(), sym.section) {
        ("", SymbolSection::Section(s)) => objects[r.obj].elf.sections[s].name.clone(),
        (name, _) => name.to_owned(),
    }
}

struct Linked<'a, 'b> {
    ctx: &'a Context<'a, 'b>,
    layout: &'a Layout,
}

impl Linked<'_, '_> {
    /// Adresse et nature de ce que vise une relocation, vue depuis la section `from`.
    fn target(&self, from: SecId, r: SymRef) -> Result<Target, LinkError> {
        let objects = self.ctx.objects;
        let referenced_from = || objects[from.0].elf.sections[from.1].name.clone();
        match self.ctx.resolve(r) {
            Resolved::UndefinedWeak => Ok(Target {
                address: 0,
                thumb: false,
                weak_undefined: true,
            }),
            Resolved::Undefined => {
                let name = display_name(objects, r);
                if name == ls::EXTERNAL_DATA_START || name == ls::EXTERNAL_DATA_SIZE {
                    return Err(LinkError::MissingExternalData);
                }
                Err(LinkError::UndefinedSymbol {
                    name,
                    referenced_from: referenced_from(),
                })
            }
            Resolved::Linker => {
                let name = &objects[r.obj].elf.symbols[r.index].name;
                let address = self.layout.symbols[name.as_str()];
                Ok(Target {
                    address,
                    thumb: false,
                    weak_undefined: false,
                })
            }
            Resolved::Def(def) => {
                let sym = &objects[def.obj].elf.symbols[def.index];
                let address = match sym.section {
                    SymbolSection::Absolute => sym.value,
                    SymbolSection::Common => self.layout.commons[sym.name.as_str()],
                    SymbolSection::Section(s) => {
                        let placement =
                            self.layout.placements.get(&(def.obj, s)).ok_or_else(|| {
                                LinkError::DiscardedSection {
                                    symbol: display_name(objects, def),
                                    section: objects[def.obj].elf.sections[s].name.clone(),
                                }
                            })?;
                        placement.vma.wrapping_add(sym.value)
                    }
                    SymbolSection::Undefined => 0,
                };
                Ok(Target {
                    address,
                    thumb: sym.is_thumb_function(),
                    weak_undefined: false,
                })
            }
        }
    }
}

/// Lie les objets : suppression des sections inutiles, placement, relocations, en-tête.
pub(super) fn run(objects: &[Obj<'_>], params: &LayoutParams) -> Result<LinkedApp, LinkError> {
    let ctx = Context {
        objects,
        globals: build_globals(objects),
    };

    // Racines : le point d'entrée `_start` et les trois sections que l'en-tête d'application lit.
    let mut roots: Vec<SecId> = Vec::new();
    let start = ctx.globals.get("_start").copied();
    match start.and_then(|r| match objects[r.obj].elf.symbols[r.index].section {
        SymbolSection::Section(s) => Some((r.obj, s)),
        _ => None,
    }) {
        Some(id) => roots.push(id),
        None => return Err(LinkError::MissingSection("_start")),
    }
    for (o, obj) in objects.iter().enumerate() {
        for (s, section) in obj.elf.sections.iter().enumerate() {
            if matches!(
                section.name.as_str(),
                API_LEVEL_SECTION | NAME_SECTION | ICON_SECTION
            ) {
                roots.push((o, s));
            }
        }
    }
    let reach = ctx.reachability(&roots);
    let layout = layout(objects, &reach, params)?;
    let linked = Linked {
        ctx: &ctx,
        layout: &layout,
    };

    // Image de la flash : on y recopie le contenu de chaque section placée, relocations appliquées.
    let mut image = vec![0u8; layout.image_len as usize];
    for (&(o, s), placement) in &layout.placements {
        let elf = &objects[o].elf;
        let section = &elf.sections[s];
        let Some(flash) = placement.flash else {
            continue;
        };
        let mut bytes = elf.section_data(s).to_vec();
        for reloc in elf.relocations(s) {
            if reloc.kind == R_ARM_NONE {
                continue;
            }
            let r = SymRef {
                obj: o,
                index: reloc.symbol as usize,
            };
            let target = linked.target((o, s), r)?;
            let place = placement.vma.wrapping_add(reloc.offset);
            relocate::apply(reloc.kind, &mut bytes, reloc.offset, place, target).map_err(
                |error| LinkError::Relocation {
                    section: section.name.clone(),
                    offset: reloc.offset,
                    symbol: display_name(objects, r),
                    error,
                },
            )?;
        }
        let start = (flash - params.flash_start) as usize;
        image[start..start + bytes.len()].copy_from_slice(&bytes);
    }

    // En-tête d'application (32 octets) : magic, niveau d'API, décalages, taille, magic.
    let start_ref = start.expect("_start vérifié plus haut");
    let entry = linked.target(roots[0], start_ref)?.address;
    let first_of = |class: Class| -> SecId {
        layout
            .placements
            .iter()
            .filter(|(_, p)| p.class == class)
            .map(|(&id, p)| (p.vma, id))
            .min()
            .map(|(_, id)| id)
            .expect("section d'en-tête vérifiée par le placement")
    };
    let offset_in_flash = |address: u32| address - params.flash_start;
    let name_id = first_of(Class::Name);
    let icon_id = first_of(Class::Icon);
    let header: [u32; 8] = {
        let mut words = [0u32; 8];
        words[0] = MAGIC;
        words[1] = u32::from_le_bytes(image[4..8].try_into().expect("4 octets"));
        words[2] = offset_in_flash(layout.placements[&name_id].vma);
        words[3] = objects[icon_id.0].elf.sections[icon_id.1].size;
        words[4] = offset_in_flash(layout.placements[&icon_id].vma);
        // Le script de link GNU lit l'adresse de `_start` sans son bit Thumb : décalage pair.
        words[5] = offset_in_flash(entry & !1);
        words[6] = offset_in_flash(layout.app_end);
        words[7] = MAGIC;
        words
    };
    for (i, word) in header.iter().enumerate() {
        if i != 1 {
            image[4 * i..4 * i + 4].copy_from_slice(&word.to_le_bytes());
        }
    }

