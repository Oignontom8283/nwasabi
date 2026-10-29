//! Lecture des sections DWARF avec `gimli`, vers les structures simples de `DebugInfo`.

use std::collections::HashMap;
use std::sync::Arc;

use gimli::{AttributeValue, EndianArcSlice, LittleEndian, Reader, UnitOffset, constants as dw};

use super::{
    DebugInfo, Encoding, Field, Function, LineRow, Location, Method, Scope, Type, TypeId, TypeKind,
    Variable, Variant, VariantPart,
};

type R = EndianArcSlice<LittleEndian>;
type Dwarf = gimli::Dwarf<R>;
type Unit = gimli::Unit<R>;
type Entry = gimli::DebuggingInformationEntry<R>;

fn err(e: gimli::Error) -> String {
    format!("DWARF : {e}")
}

pub(super) fn parse(sections: &[(&str, &[u8])]) -> Result<Option<DebugInfo>, String> {
    let get = |name: &str| -> R {
        let data: Arc<[u8]> = sections
            .iter()
            .find(|(n, _)| *n == name)
            .map_or_else(|| Arc::from(Vec::new()), |(_, d)| Arc::from(d.to_vec()));
        R::new(data, LittleEndian)
    };
    if !sections.iter().any(|(n, _)| *n == ".debug_info") {
        return Ok(None);
    }
    let dwarf: Dwarf = gimli::Dwarf::load(|id: gimli::SectionId| -> Result<R, gimli::Error> {
        Ok(get(id.name()))
    })
    .map_err(err)?;
    let mut b = Builder::default();
    let mut list = Vec::new();
    let mut headers = dwarf.units();
    while let Some(header) = headers.next().map_err(err)? {
        list.push(dwarf.unit(header).map_err(err)?);
    }
    let units = Units {
        dwarf: &dwarf,
        list,
    };
    for index in 0..units.list.len() {
        b.unit(&units, index)?;
    }
    if b.functions.is_empty() && b.globals.is_empty() {
        return Ok(None);
    }
    b.rows.sort_by_key(|r| (r.address, !r.end_sequence));
    b.functions.sort_by_key(|f| f.low);
    let frame = sections
        .iter()
        .find(|(n, _)| *n == ".debug_frame")
        .map(|(_, d)| Arc::from(d.to_vec()));
    let pointers = super::types::complete(&mut b.types);
    Ok(Some(DebugInfo {
        files: b.files,
        rows: b.rows,
        functions: b.functions,
        globals: b.globals,
        types: b.types,
        pointers,
        symbol_addresses: HashMap::new(),
        qualified_names: b.qualified_names,
        frame,
    }))
}

#[derive(Default)]
struct Builder {
    files: Vec<String>,
    file_ids: HashMap<String, u32>,
    rows: Vec<LineRow>,
    functions: Vec<Function>,
    globals: Vec<Variable>,
    types: Vec<Type>,
    type_ids: HashMap<(usize, usize), TypeId>,
    /// Le préfixe (espaces de noms, classes englobantes) de chaque type : `shapes` pour `Rect` dans `shapes`.
    prefixes: HashMap<(usize, usize), String>,
    /// Une unité écrite en C++ ou en Rust : les noms de types se lisent sans `struct`.
    qualified_names: bool,
}

/// Toutes les unités de compilation : une entrée peut en référencer une autre d'une unité différente (`DW_FORM_ref_addr`,
/// fréquent avec l'optimisation à l'édition de liens).
struct Units<'a> {
    dwarf: &'a Dwarf,
    list: Vec<Unit>,
}

impl Units<'_> {
    /// L'entrée visée par une référence : (indice de son unité, position dans l'unité).
    fn resolve(&self, current: usize, value: &AttributeValue<R>) -> Option<(usize, UnitOffset)> {
        match value {
            AttributeValue::UnitRef(off) => Some((current, *off)),
            AttributeValue::DebugInfoRef(off) => self
                .list
                .iter()
                .enumerate()
                .find_map(|(i, u)| off.to_unit_offset(&u.header).map(|o| (i, o))),
            _ => None,
        }
    }
}

fn text(dwarf: &Dwarf, unit: &Unit, value: AttributeValue<R>) -> Option<String> {
    let s = dwarf.attr_string(unit, value).ok()?;
    s.to_string_lossy().ok().map(|c| c.into_owned())
}

fn unsigned(value: &AttributeValue<R>) -> Option<u64> {
    match value {
        AttributeValue::Udata(v) => Some(*v),
        AttributeValue::Data1(v) => Some(u64::from(*v)),
        AttributeValue::Data2(v) => Some(u64::from(*v)),
        AttributeValue::Data4(v) => Some(u64::from(*v)),
        AttributeValue::Data8(v) => Some(*v),
        AttributeValue::Sdata(v) => Some(*v as u64),
        _ => None,
    }
}

fn signed(value: &AttributeValue<R>) -> Option<i64> {
    match value {
        AttributeValue::Sdata(v) => Some(*v),
        AttributeValue::Data1(v) => Some(i64::from(*v as i8)),
        AttributeValue::Data2(v) => Some(i64::from(*v as i16)),
        AttributeValue::Data4(v) => Some(i64::from(*v as i32)),
        AttributeValue::Data8(v) => Some(*v as i64),
        AttributeValue::Udata(v) => Some(*v as i64),
        _ => None,
    }
}

impl Builder {
    fn file_id(&mut self, path: String) -> u32 {
        if let Some(&id) = self.file_ids.get(&path) {
            return id;
        }
        let id = self.files.len() as u32;
        self.files.push(path.clone());
        self.file_ids.insert(path, id);
        id
    }

    fn unit(&mut self, u: &Units<'_>, index: usize) -> Result<(), String> {
        let (dwarf, unit) = (u.dwarf, &u.list[index]);
        let comp_dir = unit
            .comp_dir
            .as_ref()
            .and_then(|d| d.to_string_lossy().ok().map(|c| c.into_owned()))
            .unwrap_or_default();
        // Numéro de fichier de la table des lignes de l'unité -> numéro global.
        let mut file_map: HashMap<u64, u32> = HashMap::new();
        if let Some(program) = unit.line_program.clone() {
            // Tous les fichiers de la table (les déclarations y renvoient par numéro, même sans ligne de code).
            let header = program.header().clone();
            for index in 0..=header.file_names().len() as u64 {
                let Some(entry) = header.file(index) else {
                    continue;
                };
                let name = text(dwarf, unit, entry.path_name()).unwrap_or_default();
                let dir = entry
                    .directory(&header)
                    .and_then(|d| text(dwarf, unit, d))
                    .unwrap_or_default();
                let id = self.file_id(join_path(&comp_dir, &dir, &name));
                file_map.insert(index, id);
            }
            let mut rows = program.rows();
            let mut sequence_open = false;
            while let Some((header, row)) = rows.next_row().map_err(err)? {
                let file = match file_map.get(&row.file_index()) {
                    Some(&f) => f,
                    None => {
                        let path = match row.file(header) {
                            Some(entry) => {
                                let name = text(dwarf, unit, entry.path_name()).unwrap_or_default();
                                let dir = entry
                                    .directory(header)
                                    .and_then(|d| text(dwarf, unit, d))
                                    .unwrap_or_default();
                                join_path(&comp_dir, &dir, &name)
                            }
                            None => String::new(),
                        };
                        let id = self.file_id(path);
                        file_map.insert(row.file_index(), id);
                        id
                    }
                };
                // Une séquence qui commence à 0 est du code supprimé à l'édition de liens.
                if row.address() == 0 && !sequence_open && !row.end_sequence() {
                    continue;
                }
                sequence_open = !row.end_sequence();
                if row.address() > u64::from(u32::MAX) {
                    continue;
                }
                self.rows.push(LineRow {
                    address: row.address() as u32,
                    file,
                    line: row.line().map_or(0, |l| l.get() as u32),
                    column: match row.column() {
                        gimli::ColumnType::LeftEdge => 0,
                        gimli::ColumnType::Column(c) => c.get() as u32,
                    },
                    is_stmt: row.is_stmt(),
                    end_sequence: row.end_sequence(),
                });
            }
        }

        // C++ et Rust écrivent les noms de types sans `struct`.
        let mut root = unit.entries();
        if let Ok(Some(entry)) = root.next_dfs()
            && let Some(AttributeValue::Language(language)) = entry.attr_value(dw::DW_AT_language)
        {
            self.qualified_names |= matches!(
                language,
                dw::DW_LANG_C_plus_plus
                    | dw::DW_LANG_C_plus_plus_03
                    | dw::DW_LANG_C_plus_plus_11
                    | dw::DW_LANG_C_plus_plus_14
                    | dw::DW_LANG_Rust
            );
        }
        // Parcours des entrées : fonctions et variables globales.
        let mut cursor = unit.entries();
        // Pile des conteneurs ouverts : (profondeur, fonction/bloc en cours).
        let mut stack: Vec<Open> = Vec::new();
        // Les noms des espaces de noms et des types englobants, par profondeur, et les types à lire à la fin.
        let mut scope: Vec<(isize, String)> = Vec::new();
        // Les préfixes des types se calculent d'abord : un type peut être lu (par une fonction) avant que le parcours n'arrive à lui.
        let type_offsets = self.prefix_pass(u, index);
        while let Some(entry) = cursor.next_dfs().map_err(err)? {
            let depth = entry.depth();
            while stack.last().is_some_and(|o| o.depth >= depth) {
                let done = stack.pop().expect("pile non vide");
                self.close(done, &mut stack);
            }
            while scope.last().is_some_and(|(d, _)| *d >= depth) {
                scope.pop();
            }
            let tag = entry.tag();
            if matches!(
                tag,
                dw::DW_TAG_namespace
                    | dw::DW_TAG_structure_type
                    | dw::DW_TAG_class_type
                    | dw::DW_TAG_union_type
                    | dw::DW_TAG_enumeration_type
            ) {
                let name = entry
                    .attr_value(dw::DW_AT_name)
                    .and_then(|v| text(dwarf, unit, v))
                    .unwrap_or_default();
                scope.push((depth, name));
            }
            match tag {
                dw::DW_TAG_namespace => {
                    stack.push(Open {
                        depth,
                        kind: OpenKind::Namespace,
                    });
                }
                dw::DW_TAG_subprogram => {
                    let ranges = self.ranges(dwarf, unit, entry);
                    let Some(&(low, _)) = ranges.first() else {
                        stack.push(Open {
                            depth,
                            kind: OpenKind::Skip,
                        });
                        continue;
                    };
                    let high = ranges.iter().map(|r| r.1).max().unwrap_or(low);
                    let name = self
                        .name_of(u, index, entry, 0)
                        .unwrap_or_else(|| format!("sub_{low:08x}"));
                    let name_for_prefix = name.clone();
                    let (file, line) = self.decl(entry, &file_map);
                    let ty = self.type_attr(u, index, entry);
                    let frame_base = match entry.attr_value(dw::DW_AT_frame_base) {
                        Some(AttributeValue::Exprloc(e)) => {
                            Some(e.0.to_slice().map_err(err)?.into_owned())
                        }
                        _ => None,
                    };
                    let external = matches!(
                        entry.attr_value(dw::DW_AT_external),
                        Some(AttributeValue::Flag(true))
                    );
                    stack.push(Open {
                        depth,
                        kind: OpenKind::Function(Box::new(Function {
                            name,
                            low,
                            high,
                            file,
                            line,
                            params: Vec::new(),
                            body: Scope {
                                ranges,
                                ..Scope::default()
                            },
                            frame_base,
                            ret: ty,
                            external,
                            qualified: prefixed(&scope, &name_for_prefix),
                        })),
                    });
                }
                dw::DW_TAG_lexical_block | dw::DW_TAG_inlined_subroutine => {
                    let ranges = self.ranges(dwarf, unit, entry);
                    let kind = if stack
                        .iter()
                        .any(|o| matches!(o.kind, OpenKind::Function(_)))
                        && !ranges.is_empty()
                    {
                        OpenKind::Block(Scope {
                            ranges,
                            ..Scope::default()
                        })
                    } else {
                        OpenKind::Skip
                    };
                    stack.push(Open { depth, kind });
                }
                dw::DW_TAG_formal_parameter | dw::DW_TAG_variable => {
                    let inside_function = stack
                        .iter()
                        .any(|o| matches!(o.kind, OpenKind::Function(_)));
                    let parent_skipped = stack
                        .last()
                        .is_some_and(|o| matches!(o.kind, OpenKind::Skip));
                    if parent_skipped {
                        continue;
                    }
                    let is_param = entry.tag() == dw::DW_TAG_formal_parameter;
                    let Some(var) = self.variable(u, index, entry, &file_map, is_param)? else {
                        continue;
                    };
                    if inside_function {
                        // Une variable `static` d'une fonction (adresse fixe) reste une variable de la fonction.
                        match stack.last_mut() {
                            Some(Open {
                                kind: OpenKind::Function(f),
                                ..
                            }) => {
                                if is_param {
                                    f.params.push(var);
                                } else {
                                    f.body.vars.push(var);
                                }
                            }
                            Some(Open {
                                kind: OpenKind::Block(s),
                                ..
                            }) => s.vars.push(var),
                            _ => {}
                        }
                    } else if !is_param && stack.is_empty_or_namespace() {
                        let mut var = var;
                        var.qualified = prefixed(&scope, &var.name);
                        self.globals.push(var);
                    }
                }
                // Les types sont lus à la demande (voir `type_of`) ; on garde le niveau de profondeur des autres entrées.
                _ => {
                    if depth > 0 && stack.last().is_none_or(|o| o.depth < depth) {
                        stack.push(Open {
                            depth,
                            kind: OpenKind::Skip,
                        });
                    }
                }
            }
        }
        while let Some(done) = stack.pop() {
            self.close(done, &mut stack);
        }
        for offset in type_offsets {
            self.type_of(u, index, offset);
        }
        Ok(())
    }

    fn close(&mut self, done: Open, stack: &mut [Open]) {
        match done.kind {
            OpenKind::Function(f) => {
                // Une fonction supprimée à l'édition de liens a une adresse nulle : elle n'existe pas.
                if f.low != 0 {
                    self.functions.push(*f);
                }
            }
            OpenKind::Block(scope) => {
                if let Some(parent) = stack
                    .iter_mut()
                    .rev()
                    .find(|o| !matches!(o.kind, OpenKind::Skip | OpenKind::Namespace))
                {
                    match &mut parent.kind {
                        OpenKind::Function(f) => f.body.children.push(scope),
                        OpenKind::Block(s) => s.children.push(scope),
                        OpenKind::Skip | OpenKind::Namespace => {}
                    }
                }
            }
            OpenKind::Skip | OpenKind::Namespace => {}
        }
    }

    fn ranges(&self, dwarf: &Dwarf, unit: &Unit, entry: &Entry) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        if let Ok(mut it) = dwarf.die_ranges(unit, entry) {
            while let Ok(Some(r)) = it.next() {
                if r.begin != 0 && r.end > r.begin && r.end <= u64::from(u32::MAX) {
                    out.push((r.begin as u32, r.end as u32));
                }
            }
        }
        out
    }

    /// Le nom d'une entrée, en suivant `DW_AT_abstract_origin` et `DW_AT_specification` (instances d'une fonction en ligne, souvent
    /// dans une autre unité).
    fn name_of(&self, u: &Units<'_>, index: usize, entry: &Entry, depth: u32) -> Option<String> {
        let (dwarf, unit) = (u.dwarf, &u.list[index]);
        if let Some(v) = entry.attr_value(dw::DW_AT_name) {
            return text(dwarf, unit, v);
        }
        if depth > 4 {
            return None;
        }
        for at in [dw::DW_AT_abstract_origin, dw::DW_AT_specification] {
            if let Some((idx, off)) = entry.attr_value(at).and_then(|v| u.resolve(index, &v))
                && let Ok(origin) = u.list[idx].entry(off)
                && let Some(n) = self.name_of(u, idx, &origin, depth + 1)
            {
                return Some(n);
            }
        }
        // Dernier recours : le nom du symbole, tel que l'éditeur de liens le connaît.
        for at in [dw::DW_AT_linkage_name, dw::DW_AT_MIPS_linkage_name] {
            if let Some(v) = entry.attr_value(at) {
                return text(dwarf, unit, v);
            }
        }
        None
    }

    fn decl(&mut self, entry: &Entry, file_map: &HashMap<u64, u32>) -> (u32, u32) {
        let file = entry
            .attr_value(dw::DW_AT_decl_file)
            .and_then(|v| unsigned(&v))
            .and_then(|f| file_map.get(&f).copied())
            .unwrap_or(u32::MAX);
        let line = entry
            .attr_value(dw::DW_AT_decl_line)
            .and_then(|v| unsigned(&v))
            .unwrap_or(0) as u32;
        (file, line)
    }

    fn type_attr(&mut self, u: &Units<'_>, index: usize, entry: &Entry) -> Option<TypeId> {
        for at in [
            dw::DW_AT_type,
            dw::DW_AT_abstract_origin,
            dw::DW_AT_specification,
        ] {
            let Some((idx, off)) = entry.attr_value(at).and_then(|v| u.resolve(index, &v)) else {
                continue;
            };
            if at == dw::DW_AT_type {
                return Some(self.type_of(u, idx, off));
            }
            if let Ok(origin) = u.list[idx].entry(off)
                && let Some(t) = self.type_attr(u, idx, &origin)
            {
                return Some(t);
            }
        }
        None
    }

    fn variable(
        &mut self,
        u: &Units<'_>,
        index: usize,
        entry: &Entry,
        file_map: &HashMap<u64, u32>,
        is_param: bool,
    ) -> Result<Option<Variable>, String> {
        let (dwarf, unit) = (u.dwarf, &u.list[index]);
        // Une déclaration sans emplacement ni valeur (`extern int x;`) n'est pas une variable de l'application.
        let Some(name) = self.name_of(u, index, entry, 0) else {
            return Ok(None);
        };
        let ty = self.type_attr(u, index, entry);
        let (file, line) = self.decl(entry, file_map);
        let location = match entry.attr_value(dw::DW_AT_location) {
            None => Location::None,
            Some(AttributeValue::Exprloc(e)) => {
                Location::Expr(e.0.to_slice().map_err(err)?.into_owned())
            }
            Some(other) => {
                let mut list = Vec::new();
                if let Ok(Some(mut it)) = dwarf.attr_locations(unit, other) {
                    while let Ok(Some(item)) = it.next() {
                        if item.range.begin == 0 && item.range.end == 0 {
                            continue;
                        }
                        let bytes = item.data.0.to_slice().map_err(err)?.into_owned();
                        list.push((item.range.begin as u32, item.range.end as u32, bytes));
                    }
                }
                Location::List(list)
            }
        };
        let const_value = entry
            .attr_value(dw::DW_AT_const_value)
            .and_then(|v| signed(&v));
        if matches!(location, Location::None) && const_value.is_none() && !is_param {
            // `static` supprimée, déclaration `extern` : rien à montrer.
            return Ok(None);
        }
        Ok(Some(Variable {
            name,
            ty,
            location,
            file,
            line,
            is_param,
            qualified: None,
            const_value,
        }))
    }

    // ---- types ---------------------------------------------------------------------------------------------------

    /// Parcourt l'unité une première fois pour donner à chaque type son préfixe (espaces de noms et types englobants) ; rend les
    /// positions des types à lire.
    fn prefix_pass(&mut self, u: &Units<'_>, index: usize) -> Vec<UnitOffset> {
        let (dwarf, unit) = (u.dwarf, &u.list[index]);
        let mut scope: Vec<(isize, String)> = Vec::new();
        let mut offsets = Vec::new();
        let mut cursor = unit.entries();
        while let Ok(Some(entry)) = cursor.next_dfs() {
            let depth = entry.depth();
            while scope.last().is_some_and(|(d, _)| *d >= depth) {
                scope.pop();
            }
            let tag = entry.tag();
            let is_scope = matches!(
                tag,
                dw::DW_TAG_namespace
                    | dw::DW_TAG_structure_type
                    | dw::DW_TAG_class_type
                    | dw::DW_TAG_union_type
                    | dw::DW_TAG_enumeration_type
            );
            let is_type = is_scope && tag != dw::DW_TAG_namespace
                || matches!(tag, dw::DW_TAG_base_type | dw::DW_TAG_typedef);
            if is_type {
                let prefix: Vec<&str> = scope
                    .iter()
                    .map(|(_, n)| n.as_str())
                    .filter(|n| !n.is_empty())
                    .collect();
                if !prefix.is_empty() {
                    self.prefixes
                        .insert((index, entry.offset().0), prefix.join("::"));
                }
                // Une déclaration seule (`struct X;`) ne décrit rien : on ne la lit pas d'office.
                if !matches!(
                    entry.attr_value(dw::DW_AT_declaration),
                    Some(AttributeValue::Flag(true))
                ) {
                    offsets.push(entry.offset());
                }
            }
            if is_scope {
                let name = entry
                    .attr_value(dw::DW_AT_name)
                    .and_then(|v| text(dwarf, unit, v))
                    .unwrap_or_default();
                scope.push((depth, name));
            }
        }
        offsets
    }

    /// Les champs, les classes de base, les fonctions membres et les variantes d'une structure, d'une classe ou d'une union.
    fn struct_members(
        &mut self,
        u: &Units<'_>,
        index: usize,
        off: UnitOffset,
        entry: &Entry,
    ) -> (Vec<Field>, Vec<Method>, Option<VariantPart>) {
        let (dwarf, unit) = (u.dwarf, &u.list[index]);
        let (mut fields, mut methods, mut variants) = (Vec::new(), Vec::new(), None);
        let Ok(mut cursor) = unit.entries_at_offset(off) else {
            return (fields, methods, variants);
        };
        let _ = cursor.next_dfs();
        let base_depth = entry.depth();
        let name_of = |e: &Entry| {
            e.attr_value(dw::DW_AT_name)
                .and_then(|v| text(dwarf, unit, v))
        };
        let member_offset = |e: &Entry| -> u32 {
            match e.attr_value(dw::DW_AT_data_member_location) {
                Some(AttributeValue::Exprloc(x)) => {
                    // `DW_OP_plus_uconst N` : le seul cas qu'émettent les compilateurs actuels.
                    let bytes = x.0.to_slice().map(|c| c.into_owned()).unwrap_or_default();
                    match bytes.split_first() {
                        Some((0x23, rest)) => read_uleb(rest).unwrap_or(0) as u32,
                        _ => 0,
                    }
                }
                Some(v) => unsigned(&v).unwrap_or(0) as u32,
                None => 0,
            }
        };
        while let Ok(Some(child)) = cursor.next_dfs() {
            if child.depth() <= base_depth {
                break;
            }
            if child.depth() != base_depth + 1 {
                continue;
            }
            let child = child.clone();
            match child.tag() {
                dw::DW_TAG_member | dw::DW_TAG_inheritance => {
                    let is_base = child.tag() == dw::DW_TAG_inheritance;
                    // Un membre statique (DWARF 4) n'a pas de place dans l'objet.
                    if matches!(
                        child.attr_value(dw::DW_AT_declaration),
                        Some(AttributeValue::Flag(true))
                    ) {
                        continue;
                    }
                    let Some(fty) = self.type_attr(u, index, &child) else {
                        continue;
                    };
                    let offset = member_offset(&child);
                    let bits = child
                        .attr_value(dw::DW_AT_bit_size)
                        .and_then(|v| unsigned(&v))
                        .map(|bs| {
                            match child
                                .attr_value(dw::DW_AT_data_bit_offset)
                                .and_then(|v| unsigned(&v))
                            {
                                Some(dbo) => ((bs as u8), (dbo as u32 - offset * 8) as u8),
                                None => (bs as u8, 0),
                            }
                        });
                    let name = if is_base {
                        // Une base se nomme comme son type (`Shape`), sans son espace de noms.
                        let full = self.types.get(fty).and_then(|t| t.name.clone());
                        full.map_or_else(String::new, |n| {
                            n.rsplit("::").next().unwrap_or(&n).to_owned()
                        })
                    } else {
                        name_of(&child).unwrap_or_default()
                    };
                    fields.push(Field {
                        name,
                        ty: fty,
                        offset,
                        bits,
                        base: is_base,
                    });
                }
                dw::DW_TAG_subprogram => {
                    let Some(name) = name_of(&child) else {
                        continue;
                    };
                    let linkage = [dw::DW_AT_linkage_name, dw::DW_AT_MIPS_linkage_name]
                        .into_iter()
                        .find_map(|at| child.attr_value(at))
                        .and_then(|v| text(dwarf, unit, v));
                    let virtual_slot = match child.attr_value(dw::DW_AT_vtable_elem_location) {
                        Some(AttributeValue::Exprloc(x)) => {
                            let bytes = x.0.to_slice().map(|c| c.into_owned()).unwrap_or_default();
                            match bytes.split_first() {
                                Some((0x10, rest)) => read_uleb(rest).map(|n| n as u32),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    // Les paramètres : `this` est artificiel.
                    let (mut params, mut has_this) = (0usize, false);
                    if let Ok(mut inner) = unit.entries_at_offset(child.offset()) {
                        let _ = inner.next_dfs();
                        while let Ok(Some(p)) = inner.next_dfs() {
                            // La profondeur est relative au départ du curseur : 0 pour la fonction, 1 pour ses paramètres.
                            if p.depth() <= 0 {
                                break;
                            }
                            if p.tag() == dw::DW_TAG_formal_parameter && p.depth() == 1 {
                                if matches!(
                                    p.attr_value(dw::DW_AT_artificial),
                                    Some(AttributeValue::Flag(true))
                                ) {
                                    has_this = true;
                                } else {
                                    params += 1;
                                }
                            }
                        }
                    }
                    let ret = self.type_attr(u, index, &child);
                    methods.push(Method {
                        name,
                        linkage,
                        virtual_slot,
                        params,
                        is_static: !has_this,
                        ret,
                    });
                }
                dw::DW_TAG_variant_part => {
                    variants = self.variant_part(u, index, &child);
                }
                _ => {}
            }
        }
        (fields, methods, variants)
    }

    /// Les variantes d'une énumération Rust : l'étiquette (`DW_AT_discr`) et, pour chaque valeur, le type des champs qu'elle porte.
    fn variant_part(&mut self, u: &Units<'_>, index: usize, part: &Entry) -> Option<VariantPart> {
        let (dwarf, unit) = (u.dwarf, &u.list[index]);
        let discr_member = match part.attr_value(dw::DW_AT_discr) {
            Some(v) => u
                .resolve(index, &v)
                .and_then(|(i, o)| u.list[i].entry(o).ok().map(|e| (i, e))),
            None => None,
        };
        let mut cursor = unit.entries_at_offset(part.offset()).ok()?;
        let _ = cursor.next_dfs();
        let mut discr: Option<(u32, u32)> = discr_member.as_ref().map(|(i, e)| {
            let ty = self.type_attr(u, *i, e);
            let size = ty.map_or(1, |t| self.types[t].size.max(1));
            let offset = match e.attr_value(dw::DW_AT_data_member_location) {
                Some(v) => unsigned(&v).unwrap_or(0) as u32,
                None => 0,
            };
            (offset, size)
        });
        let mut variants = Vec::new();
        while let Ok(Some(child)) = cursor.next_dfs() {
            if child.depth() <= 0 {
                break;
            }
            if child.depth() != 1 {
                continue;
            }
            let child = child.clone();
            if child.tag() == dw::DW_TAG_member && discr.is_none() {
                let size = self
                    .type_attr(u, index, &child)
                    .map_or(1, |t| self.types[t].size.max(1));
                let offset = child
                    .attr_value(dw::DW_AT_data_member_location)
                    .and_then(|v| unsigned(&v))
                    .unwrap_or(0) as u32;
                discr = Some((offset, size));
            } else if child.tag() == dw::DW_TAG_variant {
                let value = child
                    .attr_value(dw::DW_AT_discr_value)
                    .and_then(|v| signed(&v));
                // Le membre de la variante porte son nom et le type de ses champs.
                if let Ok(mut inner) = unit.entries_at_offset(child.offset()) {
                    let _ = inner.next_dfs();
                    while let Ok(Some(member)) = inner.next_dfs() {
                        if member.depth() <= 0 {
                            break;
                        }
                        if member.tag() == dw::DW_TAG_member {
                            let member = member.clone();
                            if let Some(ty) = self.type_attr(u, index, &member) {
                                variants.push(Variant {
                                    discr: value,
                                    name: member
                                        .attr_value(dw::DW_AT_name)
                                        .and_then(|v| text(dwarf, unit, v))
                                        .unwrap_or_default(),
                                    ty,
                                    offset: member
                                        .attr_value(dw::DW_AT_data_member_location)
                                        .and_then(|v| unsigned(&v))
                                        .unwrap_or(0)
                                        as u32,
                                });
                            }
                            break;
                        }
                    }
                }
            }
        }
        let (discr_offset, discr_size) = discr?;
        Some(VariantPart {
            discr_offset,
            discr_size,
            variants,
        })
    }

    fn type_of(&mut self, u: &Units<'_>, index: usize, off: UnitOffset) -> TypeId {
        let (dwarf, unit) = (u.dwarf, &u.list[index]);
        if let Some(&id) = self.type_ids.get(&(index, off.0)) {
            return id;
        }
        let id = self.types.len();
        self.types.push(Type {
            name: None,
            size: 0,
            kind: TypeKind::Unknown,
        });
        self.type_ids.insert((index, off.0), id);
        let Ok(entry) = unit.entry(off) else {
            return id;
        };
        let name = entry
            .attr_value(dw::DW_AT_name)
            .and_then(|v| text(dwarf, unit, v))
            .map(|n| match self.prefixes.get(&(index, off.0)) {
                Some(prefix) => format!("{prefix}::{n}"),
                None => n,
            });
        let size = entry
            .attr_value(dw::DW_AT_byte_size)
            .and_then(|v| unsigned(&v))
            .unwrap_or(0) as u32;
        let target = self.type_attr(u, index, &entry);
        let kind = match entry.tag() {
            dw::DW_TAG_base_type => {
                let enc = entry.attr_value(dw::DW_AT_encoding);
                let enc = match enc {
                    Some(AttributeValue::Encoding(e)) => e,
                    _ => dw::DW_ATE_unsigned,
                };
                TypeKind::Base(match enc {
                    dw::DW_ATE_signed => Encoding::Signed,
                    dw::DW_ATE_float => Encoding::Float,
                    dw::DW_ATE_boolean => Encoding::Bool,
                    dw::DW_ATE_signed_char => Encoding::SignedChar,
                    dw::DW_ATE_unsigned_char | dw::DW_ATE_UTF => Encoding::UnsignedChar,
                    _ => Encoding::Unsigned,
                })
            }
            dw::DW_TAG_pointer_type => TypeKind::Pointer(target),
            dw::DW_TAG_reference_type | dw::DW_TAG_rvalue_reference_type => {
                TypeKind::Reference(target)
            }
            dw::DW_TAG_typedef => TypeKind::Typedef(target),
            dw::DW_TAG_const_type | dw::DW_TAG_restrict_type | dw::DW_TAG_atomic_type => {
                TypeKind::Const(target)
            }
            dw::DW_TAG_volatile_type => TypeKind::Volatile(target),
            dw::DW_TAG_structure_type | dw::DW_TAG_union_type | dw::DW_TAG_class_type => {
                let (fields, methods, variants) = self.struct_members(u, index, off, &entry);
                TypeKind::Struct {
                    fields,
                    union: entry.tag() == dw::DW_TAG_union_type,
                    methods,
                    variants,
                }
            }
            dw::DW_TAG_array_type => {
                let mut counts = Vec::new();
                if let Ok(mut cursor) = unit.entries_at_offset(off) {
                    let _ = cursor.next_dfs();
                    let base_depth = entry.depth();
                    while let Ok(Some(child)) = cursor.next_dfs() {
                        if child.depth() <= base_depth {
                            break;
                        }
                        if child.tag() == dw::DW_TAG_subrange_type {
                            let count = child
                                .attr_value(dw::DW_AT_count)
                                .and_then(|v| unsigned(&v))
                                .or_else(|| {
                                    child
                                        .attr_value(dw::DW_AT_upper_bound)
                                        .and_then(|v| signed(&v))
                                        .map(|u| (u + 1) as u64)
                                });
                            counts.push(count.map(|c| c as u32));
                        }
                    }
                }
                match target {
                    Some(elem) => TypeKind::Array { elem, counts },
                    None => TypeKind::Unknown,
                }
            }
            dw::DW_TAG_enumeration_type => {
                let mut variants = Vec::new();
                if let Ok(mut cursor) = unit.entries_at_offset(off) {
                    let _ = cursor.next_dfs();
                    let base_depth = entry.depth();
                    while let Ok(Some(child)) = cursor.next_dfs() {
                        if child.depth() <= base_depth {
                            break;
                        }
                        if child.tag() == dw::DW_TAG_enumerator
                            && let Some(n) = child
                                .attr_value(dw::DW_AT_name)
                                .and_then(|v| text(dwarf, unit, v))
                        {
                            let v = child
                                .attr_value(dw::DW_AT_const_value)
                                .and_then(|v| signed(&v))
                                .unwrap_or(0);
                            variants.push((n, v));
                        }
                    }
                }
                TypeKind::Enum {
                    variants,
                    under: target,
                }
            }
            dw::DW_TAG_subroutine_type => {
                let mut params = Vec::new();
                if let Ok(mut cursor) = unit.entries_at_offset(off) {
                    let _ = cursor.next_dfs();
                    let base_depth = entry.depth();
                    while let Ok(Some(child)) = cursor.next_dfs() {
                        if child.depth() <= base_depth {
                            break;
                        }
                        if child.tag() == dw::DW_TAG_formal_parameter {
                            let child = child.clone();
                            if let Some(t) = self.type_attr(u, index, &child) {
                                params.push(t);
                            }
                        }
                    }
                }
                TypeKind::Function {
                    ret: target,
                    params,
                }
            }
            dw::DW_TAG_unspecified_type => TypeKind::Void,
            _ => TypeKind::Unknown,
        };
        let slot = &mut self.types[id];
        slot.name = name;
        slot.size = size;
        slot.kind = kind;
        id
    }
}

enum OpenKind {
    Function(Box<Function>),
    Block(Scope),
    /// Un espace de noms : transparent, ses variables et fonctions sont des variables et fonctions globales.
    Namespace,
    Skip,
}

struct Open {
    depth: isize,
    kind: OpenKind,
}

trait StackExt {
    fn is_empty_or_namespace(&self) -> bool;
}

impl StackExt for Vec<Open> {
    fn is_empty_or_namespace(&self) -> bool {
        self.iter()
            .all(|o| matches!(o.kind, OpenKind::Skip | OpenKind::Namespace))
    }
}

fn join_path(comp_dir: &str, dir: &str, name: &str) -> String {
    let is_absolute = |p: &str| p.starts_with('/') || p.chars().nth(1) == Some(':');
    let mut joined = name.to_owned();
    for base in [dir, comp_dir] {
        if !is_absolute(&joined) && !base.is_empty() {
            joined = format!("{}/{joined}", base.trim_end_matches('/'));
        }
    }
    normalize(&joined)
}

/// Le chemin sans `.` ni `..` ni séparateurs en double (`./src/../src/a.c` devient `src/a.c`).
fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != "..") => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    if path.starts_with('/') {
        format!("/{joined}")
    } else {
        joined
    }
}

/// Un entier LEB128 non signé en tête de `bytes`.
pub(super) fn read_uleb(bytes: &[u8]) -> Option<u64> {
    let mut result = 0u64;
    let mut shift = 0;
    for &b in bytes {
        result |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
    None
}

/// `espace::nom` quand on est dans des espaces de noms ou des types ; `None` au niveau global.
fn prefixed(scope: &[(isize, String)], name: &str) -> Option<String> {
    let prefix: Vec<&str> = scope
        .iter()
        .map(|(_, n)| n.as_str())
        .filter(|n| !n.is_empty())
        .collect();
    (!prefix.is_empty()).then(|| format!("{}::{name}", prefix.join("::")))
}
