//! Types dérivés ajoutés après la lecture : types de base courants, pointeurs (sur un ou deux niveaux), tableaux à plusieurs
//! dimensions en tableaux imbriqués. Ils permettent aux expressions de convertir (`(char)n`, `(int *)p`) et d'indexer `a[i][j]`.

use std::collections::HashMap;

use super::{DebugInfo, Encoding, Type, TypeId, TypeKind};

/// Les types de base que les expressions peuvent nommer, avec leur représentation.
const STANDARD: &[(&str, Encoding, u32)] = &[
    ("char", Encoding::SignedChar, 1),
    ("signed char", Encoding::SignedChar, 1),
    ("unsigned char", Encoding::UnsignedChar, 1),
    ("short int", Encoding::Signed, 2),
    ("short unsigned int", Encoding::Unsigned, 2),
    ("int", Encoding::Signed, 4),
    ("unsigned int", Encoding::Unsigned, 4),
    ("long int", Encoding::Signed, 4),
    ("long unsigned int", Encoding::Unsigned, 4),
    ("long long int", Encoding::Signed, 8),
    ("long long unsigned int", Encoding::Unsigned, 8),
    ("float", Encoding::Float, 4),
    ("double", Encoding::Float, 8),
    ("_Bool", Encoding::Bool, 1),
];

/// Ajoute aux types lus ceux dont les expressions ont besoin. Rend la table des pointeurs.
pub(super) fn complete(types: &mut Vec<Type>) -> HashMap<Option<TypeId>, TypeId> {
    // Les tableaux à plusieurs dimensions deviennent des tableaux de tableaux.
    let mut i = 0;
    while i < types.len() {
        if let TypeKind::Array { elem, counts } = &types[i].kind
            && counts.len() > 1
        {
            let (elem, counts) = (*elem, counts.clone());
            let inner = types.len();
            types.push(Type {
                name: None,
                size: 0,
                kind: TypeKind::Array {
                    elem,
                    counts: counts[1..].to_vec(),
                },
            });
            types[i].kind = TypeKind::Array {
                elem: inner,
                counts: vec![counts[0]],
            };
        }
        i += 1;
    }
    // Les types de base courants.
    for (name, encoding, size) in STANDARD {
        let present = types.iter().any(|t| {
            matches!(&t.kind, TypeKind::Base(e) if e == encoding)
                && t.size == *size
                && t.name.as_deref() == Some(name)
        });
        if !present {
            types.push(Type {
                name: Some((*name).to_owned()),
                size: *size,
                kind: TypeKind::Base(*encoding),
            });
        }
    }
    // Les pointeurs : sur `void`, sur chaque type, puis sur chaque pointeur (deux niveaux).
    let mut pointers: HashMap<Option<TypeId>, TypeId> = HashMap::new();
    for (id, t) in types.iter().enumerate() {
        if let TypeKind::Pointer(target) = t.kind {
            pointers.entry(target).or_insert(id);
        }
    }
    for round in 0..2 {
        let count = types.len();
        let targets: Vec<Option<TypeId>> = if round == 0 {
            std::iter::once(None).chain((0..count).map(Some)).collect()
        } else {
            pointers.values().copied().map(Some).collect::<Vec<_>>()
        };
        for target in targets {
            if let std::collections::hash_map::Entry::Vacant(slot) = pointers.entry(target) {
                slot.insert(types.len());
                types.push(Type {
                    name: None,
                    size: 4,
                    kind: TypeKind::Pointer(target),
                });
            }
        }
    }
    pointers
}

impl DebugInfo {
    /// Le type « pointeur sur `target` » (`None` : `void *`), s'il existe parmi ceux que l'on connaît.
    pub fn pointer_to(&self, target: Option<TypeId>) -> Option<TypeId> {
        self.pointers.get(&target).copied()
    }

    /// Le type de base de ce nom en C (`unsigned int`, `uint8_t`, `u32`…), ou à défaut un de même représentation.
    pub fn base_named(&self, words: &[&str]) -> Option<TypeId> {
        let has = |w: &str| words.contains(&w);
        let longs = words.iter().filter(|w| **w == "long").count();
        let unsigned = has("unsigned");
        let (name, encoding, size): (&str, Encoding, u32) = if has("float") {
            ("float", Encoding::Float, 4)
        } else if has("double") {
            ("double", Encoding::Float, 8)
        } else if has("_Bool") || has("bool") {
            ("_Bool", Encoding::Bool, 1)
        } else if has("char") {
            match (unsigned, has("signed")) {
                (true, _) => ("unsigned char", Encoding::UnsignedChar, 1),
                (false, true) => ("signed char", Encoding::SignedChar, 1),
                _ => ("char", Encoding::SignedChar, 1),
            }
        } else if has("short") {
            if unsigned {
                ("short unsigned int", Encoding::Unsigned, 2)
            } else {
                ("short int", Encoding::Signed, 2)
            }
        } else if longs >= 2 {
            if unsigned {
                ("long long unsigned int", Encoding::Unsigned, 8)
            } else {
                ("long long int", Encoding::Signed, 8)
            }
        } else if longs == 1 {
            if unsigned {
                ("long unsigned int", Encoding::Unsigned, 4)
            } else {
                ("long int", Encoding::Signed, 4)
            }
        } else if has("int") || unsigned || has("signed") {
            if unsigned {
                ("unsigned int", Encoding::Unsigned, 4)
            } else {
                ("int", Encoding::Signed, 4)
            }
        } else {
            return None;
        };
        self.types
            .iter()
            .position(|t| {
                t.name.as_deref() == Some(name)
                    && matches!(&t.kind, TypeKind::Base(e) if *e == encoding)
            })
            .or_else(|| {
                self.types.iter().position(|t| {
                    t.size == size && matches!(&t.kind, TypeKind::Base(e) if *e == encoding)
                })
            })
    }

    /// Le type nommé `name` : un `typedef`, un type de base (`i32`, `uint8_t`…), ou avec `kind` (`struct`, `union`, `enum`) une structure, une
    /// union ou une énumération.
    pub fn type_named(&self, name: &str, kind: Option<&str>) -> Option<TypeId> {
        // Les espaces ne comptent pas (`Stack<int, 4>` s'écrit aussi `Stack<int,4>`) ; la fin d'un nom complet suffit (`Rect`).
        let plain = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let wanted = plain(name);
        let tail = format!("::{wanted}");
        let matches = |t: &Type| {
            t.name.as_deref().is_some_and(|n| {
                let n = plain(n);
                n == wanted || n.ends_with(&tail)
            })
        };
        // Un nom complet exact l'emporte sur une fin de nom.
        let exact = |t: &Type| t.name.as_deref().map(plain).as_deref() == Some(wanted.as_str());
        let accepts = |t: &Type| match (&t.kind, kind) {
            (TypeKind::Struct { union, .. }, Some("struct")) => !union,
            (TypeKind::Struct { union, .. }, Some("union")) => *union,
            (TypeKind::Enum { .. }, Some("enum")) => true,
            (TypeKind::Typedef(_) | TypeKind::Base(_), None) => true,
            (TypeKind::Struct { .. } | TypeKind::Enum { .. }, None) => true,
            // Les noms de C++ et de Rust s'écrivent aussi avec `struct` / `class`.
            (TypeKind::Struct { .. }, Some("class")) => true,
            _ => false,
        };
        let usable = |t: &Type| accepts(t) && !is_empty_declaration(t);
        self.types
            .iter()
            .position(|t| exact(t) && usable(t))
            .or_else(|| self.types.iter().position(|t| matches(t) && usable(t)))
    }
}

/// Une déclaration sans contenu (`struct X;` vue d'un autre fichier) : elle ne décrit rien.
fn is_empty_declaration(t: &Type) -> bool {
    matches!(&t.kind, TypeKind::Struct { fields, variants: None, methods, .. } if fields.is_empty() && methods.is_empty() && t.size == 0)
}
