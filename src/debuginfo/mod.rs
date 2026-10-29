//! Informations de débogage du code source (DWARF) : lignes, fonctions, variables, types, pile.
//!
//! Elles viennent des sections `.debug_*` du `.nwa`, que l'édition de liens rend avec les adresses définitives
//! (voir `LinkedApp::debug`). Ce module les lit une fois, dans des structures simples ; les requêtes
//! (adresse → ligne, ligne → adresses, variables visibles à une adresse, pile d'appels) n'ont plus besoin de `gimli`.

mod eval;
mod expr;
mod parse;
mod types;
mod unwind;

pub use eval::{CallArg, CallResult, Context, Place, Target, Value, ValueKind, locate, read_value};
pub use expr::{EvalError, evaluate};
pub use unwind::Frame;

use std::sync::Arc;

pub type TypeId = usize;

/// Une ligne de la table des lignes : à partir de `address`, le code vient de `file`:`line`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRow {
    pub address: u32,
    pub file: u32,
    pub line: u32,
    pub column: u32,
    /// Début d'une instruction de source : c'est là qu'un point d'arrêt de ligne se pose.
    pub is_stmt: bool,
    /// Fin d'une séquence : `address` est la première adresse après le code.
    pub end_sequence: bool,
}

/// Où se trouve une variable : une expression DWARF, ou plusieurs selon l'adresse (variable optimisée).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    None,
    Expr(Vec<u8>),
    /// `(début, fin, expression)` : valable pour `début <= pc < fin`.
    List(Vec<(u32, u32, Vec<u8>)>),
}

impl Location {
    /// L'expression valable à `pc`.
    pub fn at(&self, pc: u32) -> Option<&[u8]> {
        match self {
            Self::None => None,
            Self::Expr(e) => Some(e),
            Self::List(list) => list
                .iter()
                .find(|(lo, hi, _)| *lo <= pc && pc < *hi)
                .map(|(_, _, e)| e.as_slice()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub ty: Option<TypeId>,
    pub location: Location,
    pub file: u32,
    pub line: u32,
    pub is_param: bool,
    /// Le nom complet d'une variable globale C++ ou Rust (`shapes::Shape::counter`, `main::GLOBAL_TOTAL`), tiré de son symbole.
    pub qualified: Option<String>,
    /// Valeur connue à la compilation (`DW_AT_const_value`), si la variable n'a pas d'emplacement.
    pub const_value: Option<i64>,
}

/// Un bloc de code `{ … }` : ses variables ne sont visibles que dans ses adresses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub ranges: Vec<(u32, u32)>,
    pub vars: Vec<Variable>,
    pub children: Vec<Scope>,
}

impl Scope {
    fn contains(&self, pc: u32) -> bool {
        self.ranges.iter().any(|(lo, hi)| *lo <= pc && pc < *hi)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub low: u32,
    pub high: u32,
    pub file: u32,
    pub line: u32,
    pub params: Vec<Variable>,
    /// Les variables locales (et les blocs imbriqués).
    pub body: Scope,
    /// Expression qui donne l'adresse de base des variables (`DW_AT_frame_base`).
    pub frame_base: Option<Vec<u8>>,
    pub ret: Option<TypeId>,
    pub external: bool,
    /// Le nom complet d'une fonction C++ ou Rust (`Chunk::generate`, `numcraft::world::Chunk::generate`), tiré de son symbole.
    pub qualified: Option<String>,
}

impl Function {
    /// Le nom à afficher : complet s'il est connu.
    pub fn display_name(&self) -> &str {
        self.qualified.as_deref().unwrap_or(&self.name)
    }

    /// Cette fonction porte-t-elle ce nom ? (le nom complet, ou sa fin : `generate` ou `Chunk::generate`)
    pub fn is_named(&self, wanted: &str) -> bool {
        self.name == wanted
            || self
                .qualified
                .as_deref()
                .is_some_and(|q| q == wanted || q.ends_with(&format!("::{wanted}")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Signed,
    Unsigned,
    Float,
    Bool,
    SignedChar,
    UnsignedChar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: TypeId,
    pub offset: u32,
    /// Champ de bits : taille et position (depuis le bit de poids faible du mot qui commence à `offset`).
    pub bits: Option<(u8, u8)>,
    /// Une classe de base (héritage C++) : le champ porte le nom de la base et contient ses propres champs.
    pub base: bool,
}

/// Une fonction membre d'une classe ou d'une structure (C++ et Rust).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    pub name: String,
    /// Le nom du symbole (manglé) : l'adresse de la fonction s'en déduit.
    pub linkage: Option<String>,
    /// Place dans la table des fonctions virtuelles, pour un appel dynamique.
    pub virtual_slot: Option<u32>,
    /// Nombre de paramètres sans `this`.
    pub params: usize,
    pub is_static: bool,
    pub ret: Option<TypeId>,
}

/// Les variantes d'une énumération Rust (ou d'une union étiquetée) : l'étiquette dit laquelle est active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantPart {
    pub discr_offset: u32,
    pub discr_size: u32,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// La valeur de l'étiquette ; `None` : la variante par défaut (les autres valeurs).
    pub discr: Option<i64>,
    pub name: String,
    /// Le type qui décrit les champs de la variante, et sa position dans l'énumération.
    pub ty: TypeId,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Unknown,
    Void,
    Base(Encoding),
    Pointer(Option<TypeId>),
    Struct {
        fields: Vec<Field>,
        union: bool,
        methods: Vec<Method>,
        variants: Option<VariantPart>,
    },
    /// Une référence C++ ou Rust (`int &`, `&T`) : un pointeur que l'on traverse toujours.
    Reference(Option<TypeId>),
    Array {
        elem: TypeId,
        counts: Vec<Option<u32>>,
    },
    Enum {
        variants: Vec<(String, i64)>,
        under: Option<TypeId>,
    },
    Typedef(Option<TypeId>),
    Const(Option<TypeId>),
    Volatile(Option<TypeId>),
    Function {
        ret: Option<TypeId>,
        params: Vec<TypeId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub name: Option<String>,
    pub size: u32,
    pub kind: TypeKind,
}

/// Les informations de débogage d'une application.
#[derive(Debug, Clone)]
pub struct DebugInfo {
    /// Chemins des fichiers de code source (indice = numéro de fichier des lignes).
    pub files: Vec<String>,
    rows: Vec<LineRow>,
    pub functions: Vec<Function>,
    pub globals: Vec<Variable>,
    types: Vec<Type>,
    /// Le type « pointeur sur T » pour chaque type connu (`None` : `void *`).
    pointers: std::collections::HashMap<Option<TypeId>, TypeId>,
    /// Les adresses des fonctions et variables par nom de symbole (manglé) : pour appeler une fonction membre.
    pub(crate) symbol_addresses: std::collections::HashMap<String, u32>,
    /// Le programme est en C++ ou en Rust : les noms de types s'écrivent sans `struct`.
    pub(crate) qualified_names: bool,
    /// La section `.debug_frame`, pour dérouler la pile.
    frame: Option<Arc<[u8]>>,
}

impl DebugInfo {
    /// Lit les sections `.debug_*` (nom, contenu). Rend `None` quand elles ne contiennent aucune fonction.
    pub fn from_sections(sections: &[(&str, &[u8])]) -> Result<Option<Self>, String> {
        parse::parse(sections)
    }

    pub fn ty(&self, id: TypeId) -> &Type {
        &self.types[id]
    }

    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    /// Le nom d'un type, comme en C (`int`, `Point`, `char *`, `uint8_t [3]`).
    pub fn type_name(&self, id: Option<TypeId>) -> String {
        let Some(id) = id else {
            return "void".to_owned();
        };
        self.type_name_depth(id, 0)
    }

    fn type_name_depth(&self, id: TypeId, depth: u32) -> String {
        let t = &self.types[id];
        if depth > 12 {
            return "…".to_owned();
        }
        let inner = |this: &Self, target: Option<TypeId>| match target {
            Some(t) => this.type_name_depth(t, depth + 1),
            None => "void".to_owned(),
        };
        match &t.kind {
            TypeKind::Pointer(target) => {
                let name = inner(self, *target);
                if name.ends_with('*') {
                    format!("{name}*")
                } else {
                    format!("{name} *")
                }
            }
            TypeKind::Reference(target) => format!("{} &", inner(self, *target)),
            TypeKind::Const(Some(target))
                if matches!(self.types[*target].kind, TypeKind::Pointer(_)) =>
            {
                format!("{} const", self.type_name_depth(*target, depth + 1))
            }
            TypeKind::Const(target) => format!("const {}", inner(self, *target)),
            TypeKind::Volatile(target) => format!("volatile {}", inner(self, *target)),
            TypeKind::Array { elem, counts } => {
                // Les dimensions d'un tableau de tableaux s'écrivent à la suite : `int [3][4]`.
                let mut dims: String = counts
                    .iter()
                    .map(|c| c.map_or("[]".to_owned(), |n| format!("[{n}]")))
                    .collect();
                let mut inner = *elem;
                while let TypeKind::Array { elem: next, counts } = &self.types[inner].kind {
                    dims += &counts
                        .iter()
                        .map(|c| c.map_or("[]".to_owned(), |n| format!("[{n}]")))
                        .collect::<String>();
                    inner = *next;
                }
                format!("{} {dims}", self.type_name_depth(inner, depth + 1))
            }
            TypeKind::Function { ret, params } => {
                let list: Vec<String> = params
                    .iter()
                    .map(|p| self.type_name_depth(*p, depth + 1))
                    .collect();
                format!("{} ({})", inner(self, *ret), list.join(", "))
            }
            // C++ et Rust écrivent le nom seul (`shapes::Rect`) ; C écrit `struct Item`.
            TypeKind::Struct { union, .. } => match (&t.name, self.qualified_names) {
                (Some(name), true) => name.clone(),
                (name, _) => format!(
                    "{} {}",
                    if *union { "union" } else { "struct" },
                    name.as_deref().unwrap_or("{…}")
                ),
            },
            TypeKind::Enum { .. } if self.qualified_names && t.name.is_some() => {
                t.name.clone().unwrap_or_default()
            }
            TypeKind::Enum { .. } => format!("enum {}", t.name.as_deref().unwrap_or("{…}")),
            _ => t.name.clone().unwrap_or_else(|| "?".to_owned()),
        }
    }

    /// Enlève les `typedef`, `const` et `volatile` pour atteindre le type qui décrit la représentation.
    pub fn strip(&self, mut id: TypeId) -> TypeId {
        for _ in 0..32 {
            match &self.types[id].kind {
                TypeKind::Typedef(Some(t))
                | TypeKind::Const(Some(t))
                | TypeKind::Volatile(Some(t)) => id = *t,
                _ => break,
            }
        }
        id
    }

    /// La taille d'un type, en octets (les tableaux et les `typedef` sont résolus).
    pub fn size_of(&self, id: TypeId) -> u32 {
        let s = self.strip(id);
        let t = &self.types[s];
        match &t.kind {
            TypeKind::Array { elem, counts } => {
                let n: u32 = counts.iter().map(|c| c.unwrap_or(0)).product();
                n * self.size_of(*elem)
            }
            TypeKind::Pointer(_) => 4,
            _ => t.size,
        }
    }

    // ---- lignes --------------------------------------------------------------------------------------------------

    pub fn rows(&self) -> &[LineRow] {
        &self.rows
    }

    /// La ligne de source qui contient `address`.
    pub fn line_at(&self, address: u32) -> Option<&LineRow> {
        let index = self.rows.partition_point(|r| r.address <= address);
        let row = self.rows.get(index.checked_sub(1)?)?;
        // La dernière ligne d'une séquence n'a pas de suite : `end_sequence` ferme la plage.
        (!row.end_sequence).then_some(row)
    }

    /// Les adresses où poser un point d'arrêt pour `file:line` : celles de la première ligne qui a du code à partir de `line`
    /// (comme GDB), une adresse par bloc de code contigu (une ligne de boucle `for` en a souvent deux).
    pub fn addresses_for_line(&self, file: u32, line: u32) -> (u32, Vec<u32>) {
        let mut best: Option<u32> = None;
        for r in self
            .rows
            .iter()
            .filter(|r| r.file == file && r.is_stmt && !r.end_sequence)
        {
            if r.line >= line && best.is_none_or(|b| r.line < b) {
                best = Some(r.line);
            }
        }
        let Some(found) = best else {
            return (line, Vec::new());
        };
        let mut out: Vec<u32> = Vec::new();
        let mut previous_matched = false;
        for r in &self.rows {
            let matches = r.file == file && r.line == found && r.is_stmt && !r.end_sequence;
            if matches && !previous_matched {
                out.push(r.address);
            }
            previous_matched = matches;
        }
        (found, out)
    }

    /// L'adresse où poser un point d'arrêt « au début de la fonction » : après le prologue (la deuxième ligne de la fonction).
    pub fn after_prologue(&self, f: &Function) -> u32 {
        let mut seen_first = false;
        for r in self
            .rows
            .iter()
            .filter(|r| r.address >= f.low && r.address < f.high && r.is_stmt)
        {
            if !seen_first {
                seen_first = true;
            } else if r.address > f.low {
                return r.address;
            }
        }
        f.low
    }

    /// Le numéro de fichier d'un chemin donné en entier ou par son nom (`main.c`, `src/main.c`).
    pub fn find_file(&self, name: &str) -> Option<u32> {
        let wanted = name.replace('\\', "/");
        let mut best: Option<(usize, u32)> = None;
        for (i, f) in self.files.iter().enumerate() {
            let f = f.replace('\\', "/");
            if f == wanted
                || f.ends_with(&format!("/{wanted}"))
                || wanted.ends_with(&format!("/{f}"))
            {
                let score = if f == wanted {
                    usize::MAX
                } else {
                    wanted.len()
                };
                if best.is_none_or(|(s, _)| score > s) {
                    best = Some((score, i as u32));
                }
            }
        }
        best.map(|(_, i)| i)
    }

    // ---- fonctions et variables ----------------------------------------------------------------------------------

    /// La fonction dont le code contient `pc`.
    pub fn function_at(&self, pc: u32) -> Option<&Function> {
        self.functions.iter().find(|f| f.low <= pc && pc < f.high)
    }

    pub fn function_named(&self, name: &str) -> Option<&Function> {
        // Le nom exact d'abord (`main`), puis le nom complet ou sa fin (`Chunk::generate`).
        self.functions
            .iter()
            .find(|f| f.name == name || f.qualified.as_deref() == Some(name))
            .or_else(|| self.functions.iter().find(|f| f.is_named(name)))
    }

    /// Parmi des fonctions de même nom (surcharges), celle qui convient le mieux à ces arguments : même nombre de paramètres, puis
    /// mêmes genres (flottant ou entier).
    pub fn best_overload<'a>(
        &'a self,
        candidates: &[&'a Function],
        args: &[bool],
    ) -> Option<&'a Function> {
        let is_float = |ty: Option<TypeId>| {
            ty.is_some_and(|t| {
                matches!(self.ty(self.strip(t)).kind, TypeKind::Base(Encoding::Float))
            })
        };
        candidates.iter().copied().max_by_key(|f| {
            // `this` (artificiel) n'est pas dans `args` quand on appelle une fonction libre ; on compare les derniers paramètres.
            let params: Vec<bool> = f.params.iter().map(|p| is_float(p.ty)).collect();
            let offset = params.len().saturating_sub(args.len());
            let same_count = params.len() == args.len() || params.len() == args.len() + 1;
            let matching = args
                .iter()
                .zip(&params[offset..])
                .filter(|(a, p)| a == p)
                .count();
            (same_count, matching, std::cmp::Reverse(f.low))
        })
    }

    /// La fonction dont le code commence à `address`.
    pub fn function_at_start(&self, address: u32) -> Option<&Function> {
        self.functions.iter().find(|f| f.low == address & !1)
    }

    /// Toutes les fonctions de ce nom (les surcharges du C++) : de préférence celles dont le nom est exactement celui demandé.
    pub fn functions_named(&self, name: &str) -> Vec<&Function> {
        let exact: Vec<&Function> = self
            .functions
            .iter()
            .filter(|f| f.name == name || f.qualified.as_deref() == Some(name))
            .collect();
        if !exact.is_empty() {
            return exact;
        }
        self.functions.iter().filter(|f| f.is_named(name)).collect()
    }

    /// La fonction membre `name` de la classe `ty` (ou d'une de ses bases), avec le décalage de la base où elle se trouve.
    pub fn find_method(&self, ty: TypeId, name: &str, args: usize) -> Option<(&Method, u32)> {
        let TypeKind::Struct {
            fields, methods, ..
        } = &self.ty(self.strip(ty)).kind
        else {
            return None;
        };
        let by_name = || methods.iter().filter(|m| m.name == name);
        if let Some(m) = by_name()
            .find(|m| m.params == args)
            .or_else(|| by_name().next())
        {
            return Some((m, 0));
        }
        fields.iter().filter(|f| f.base).find_map(|f| {
            self.find_method(f.ty, name, args)
                .map(|(m, o)| (m, o + f.offset))
        })
    }

    /// La fonction qui implémente la méthode `name` du type nommé `type_name`, quand le type n'en liste pas (les méthodes d'un
    /// `impl` Rust, d'une classe hors de sa déclaration) : d'après le nom complet de la fonction.
    pub fn method_function(&self, type_name: &str, name: &str, args: usize) -> Option<&Function> {
        let plain = format!("{type_name}::{name}");
        let nested = format!("::{plain}");
        let as_trait = format!("<{type_name} as ");
        let suffix = format!(">::{name}");
        let candidates: Vec<&Function> = self
            .functions
            .iter()
            .filter(|f| {
                f.qualified.as_deref().is_some_and(|q| {
                    q == plain
                        || q.ends_with(&nested)
                        || (q.starts_with(&as_trait) && q.ends_with(&suffix))
                })
            })
            .collect();
        candidates
            .iter()
            .copied()
            .find(|f| f.params.len() == args + 1 || f.params.len() == args)
            .or_else(|| candidates.first().copied())
    }

    /// L'adresse de la fonction membre `method` (par sa table virtuelle si elle est virtuelle) de l'objet à `object`.
    pub fn method_address(&self, method: &Method) -> Option<u32> {
        method
            .linkage
            .as_ref()
            .and_then(|l| self.symbol_addresses.get(l))
            .map(|a| a & !1)
    }

    /// Donne leur nom complet aux fonctions C++ et Rust, d'après les symboles de l'application (adresse, nom manglé).
    pub fn attach_symbols<'a>(&mut self, symbols: impl IntoIterator<Item = (u32, &'a str)>) {
        let symbols: Vec<(u32, &'a str)> = symbols.into_iter().collect();
        self.symbol_addresses = symbols.iter().map(|(a, n)| ((*n).to_owned(), *a)).collect();
        let by_address: std::collections::HashMap<u32, &str> =
            symbols.iter().map(|(a, n)| (a & !1, *n)).collect();
        // Une variable globale : son nom complet vient du symbole qui est à son adresse (`DW_OP_addr`).
        for v in &mut self.globals {
            if let Location::Expr(e) = &v.location
                && e.len() == 5
                && e[0] == 0x03
            {
                let address = u32::from_le_bytes([e[1], e[2], e[3], e[4]]);
                if let Some(name) = by_address.get(&address) {
                    let q = crate::symbols::qualified(name);
                    if q != *name && q != v.name {
                        v.qualified = Some(q);
                    }
                }
            }
        }
        for f in &mut self.functions {
            if let Some(name) = by_address.get(&f.low) {
                let q = crate::symbols::qualified(name);
                if q != *name && q != f.name {
                    f.qualified = Some(q);
                }
            }
        }
    }

    /// Les variables locales visibles à `pc`, des plus proches (bloc le plus imbriqué) aux plus éloignées ; les
    /// paramètres viennent à la fin.
    pub fn visible_locals(&self, pc: u32) -> Vec<&Variable> {
        let Some(f) = self.function_at(pc) else {
            return Vec::new();
        };
        let mut chain: Vec<&Scope> = vec![&f.body];
        let mut current = &f.body;
        while let Some(child) = current.children.iter().find(|c| c.contains(pc)) {
            chain.push(child);
            current = child;
        }
        let mut out: Vec<&Variable> = Vec::new();
        for scope in chain.iter().rev() {
            out.extend(scope.vars.iter());
        }
        out.extend(f.params.iter());
        out
    }

    /// Une variable par son nom, à `pc` : locale, paramètre, puis globale.
    pub fn find_variable(&self, pc: u32, name: &str) -> Option<&Variable> {
        let named = |v: &&Variable| v.name == name || v.qualified.as_deref() == Some(name);
        self.visible_locals(pc)
            .into_iter()
            .find(|v| v.name == name)
            .or_else(|| self.globals.iter().find(named))
            // La fin d'un nom complet (`counter`, `Shape::counter`).
            .or_else(|| {
                self.globals.iter().find(|v| {
                    v.qualified
                        .as_deref()
                        .is_some_and(|q| q.ends_with(&format!("::{name}")))
                })
            })
    }

    // ---- la pile ------------------------------------------------------------------------------------------------

    /// La section `.debug_frame` (vide si l'application n'en a pas).
    pub(crate) fn frame_section(&self) -> Option<&Arc<[u8]>> {
        self.frame.as_ref()
    }
}
