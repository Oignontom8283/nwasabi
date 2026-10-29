//! Évaluation des expressions d'emplacement DWARF, lecture des valeurs et mise en forme.

use super::{DebugInfo, Encoding, TypeId, TypeKind};
use crate::json::{Value as Json, ValueExt};

/// L'appareil que les expressions lisent et, pour les affectations et les appels de fonction, modifient.
pub trait Target {
    /// Lit `len` octets à `address`, ou rien si l'adresse n'est pas lisible.
    fn read(&mut self, address: u32, len: usize) -> Option<Vec<u8>>;
    /// Écrit des octets ; faux si l'adresse n'est pas inscriptible.
    fn write(&mut self, _address: u32, _data: &[u8]) -> bool {
        false
    }
    /// Modifie un registre du cadre en cours (r0 à r15).
    fn set_register(&mut self, _index: usize, _value: u32) -> bool {
        false
    }
    /// Appelle la fonction à `function` avec ces arguments (convention ARM à virgule flottante matérielle).
    fn call(&mut self, _function: u32, _args: &[CallArg]) -> Result<CallResult, String> {
        Err("les appels de fonction ne sont pas possibles ici".to_owned())
    }
}

/// Un argument d'un appel de fonction : entier ou pointeur (r0 à r3, puis la pile), flottant (s0…) ou double (d0…).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CallArg {
    Int(u32),
    Float(f32),
    Double(f64),
}

/// Ce qu'une fonction rend : r0 et r1 (entiers, pointeurs, entiers de 64 bits), s0 et s1 (flottant, ou double dans d0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallResult {
    pub r0: u32,
    pub r1: u32,
    pub s0: u32,
    pub s1: u32,
}

/// L'état dans lequel une expression s'évalue : les registres du cadre, son adresse de base, la mémoire.
pub struct Context<'a> {
    /// r0 à r15 (r13 = sp, r14 = lr, r15 = pc).
    pub regs: [u32; 16],
    /// s0 à s31 (bits).
    pub fregs: [u32; 32],
    /// Valeur de la CFA (adresse de la pile à l'entrée de la fonction) : `DW_OP_call_frame_cfa`.
    pub cfa: Option<u32>,
    /// Adresse de base des variables de la fonction : `DW_OP_fbreg`.
    pub frame_base: Option<u32>,
    pub memory: &'a mut dyn Target,
}

/// Où se trouve une valeur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Address(u32),
    /// Registre DWARF (0 à 15 : r0 à r15 ; 64 à 95 : s0 à s31 ; 256 à 287 : d0 à d31).
    Register(u16),
    /// Valeur calculée (`DW_OP_stack_value`), octets de poids faible en premier.
    Computed(Vec<u8>),
    /// Valeur répartie : `(où, taille en octets)`.
    Pieces(Vec<(Place, u32)>),
    OptimizedOut,
}

fn uleb(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut result = 0u64;
    let mut shift = 0;
    loop {
        let b = *bytes.get(*at)?;
        *at += 1;
        result |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

fn sleb(bytes: &[u8], at: &mut usize) -> Option<i64> {
    let mut result = 0i64;
    let mut shift = 0;
    loop {
        let b = *bytes.get(*at)?;
        *at += 1;
        result |= i64::from(b & 0x7f) << shift;
        shift += 7;
        if b & 0x80 == 0 {
            if shift < 64 && b & 0x40 != 0 {
                result |= -1i64 << shift;
            }
            return Some(result);
        }
        if shift > 63 {
            return None;
        }
    }
}

fn fixed<const N: usize>(bytes: &[u8], at: &mut usize) -> Option<[u8; N]> {
    let slice = bytes.get(*at..*at + N)?;
    *at += N;
    slice.try_into().ok()
}

/// Évalue une expression d'emplacement.
pub fn locate(expr: &[u8], ctx: &mut Context<'_>) -> Result<Place, String> {
    let mut stack: Vec<u64> = Vec::new();
    let mut pieces: Vec<(Place, u32)> = Vec::new();
    let mut register: Option<u16> = None;
    let mut computed = false;
    let mut implicit: Option<Vec<u8>> = None;
    let mut at = 0usize;
    let bad = || "expression d'emplacement illisible".to_owned();
    let pop = |stack: &mut Vec<u64>| {
        stack
            .pop()
            .ok_or_else(|| "pile d'expression vide".to_owned())
    };
    let mut steps = 0;
    while at < expr.len() {
        steps += 1;
        if steps > 4096 {
            return Err("expression trop longue".into());
        }
        let op = expr[at];
        at += 1;
        match op {
            0x03 => stack.push(u64::from(u32::from_le_bytes(
                fixed(expr, &mut at).ok_or_else(bad)?,
            ))),
            0x06 => {
                let a = pop(&mut stack)? as u32;
                let bytes = ctx
                    .memory
                    .read(a, 4)
                    .ok_or_else(|| format!("mémoire illisible à 0x{a:08x}"))?;
                stack.push(u64::from(u32::from_le_bytes(
                    bytes.try_into().map_err(|_| bad())?,
                )));
            }
            0x08 => stack.push(u64::from(fixed::<1>(expr, &mut at).ok_or_else(bad)?[0])),
            0x09 => {
                stack.push(i64::from(fixed::<1>(expr, &mut at).ok_or_else(bad)?[0] as i8) as u64)
            }
            0x0a => stack.push(u64::from(u16::from_le_bytes(
                fixed(expr, &mut at).ok_or_else(bad)?,
            ))),
            0x0b => stack
                .push(i64::from(i16::from_le_bytes(fixed(expr, &mut at).ok_or_else(bad)?)) as u64),
            0x0c => stack.push(u64::from(u32::from_le_bytes(
                fixed(expr, &mut at).ok_or_else(bad)?,
            ))),
            0x0d => stack
                .push(i64::from(i32::from_le_bytes(fixed(expr, &mut at).ok_or_else(bad)?)) as u64),
            0x0e | 0x0f => stack.push(u64::from_le_bytes(fixed(expr, &mut at).ok_or_else(bad)?)),
            0x10 => stack.push(uleb(expr, &mut at).ok_or_else(bad)?),
            0x11 => stack.push(sleb(expr, &mut at).ok_or_else(bad)? as u64),
            0x12 => stack.push(*stack.last().ok_or_else(bad)?),
            0x13 => {
                pop(&mut stack)?;
            }
            0x14 => stack.push(*stack.iter().rev().nth(1).ok_or_else(bad)?),
            0x15 => {
                let i = fixed::<1>(expr, &mut at).ok_or_else(bad)?[0] as usize;
                stack.push(*stack.iter().rev().nth(i).ok_or_else(bad)?);
            }
            0x16 => {
                let n = stack.len();
                if n < 2 {
                    return Err(bad());
                }
                stack.swap(n - 1, n - 2);
            }
            0x17 => {
                let n = stack.len();
                if n < 3 {
                    return Err(bad());
                }
                let top = stack.remove(n - 1);
                stack.insert(n - 3, top);
            }
            0x19 => {
                let v = pop(&mut stack)? as i32;
                stack.push(v.unsigned_abs().into());
            }
            0x1a..=0x27 => {
                if op == 0x20 || op == 0x1f {
                    let v = pop(&mut stack)? as i32;
                    stack.push(
                        if op == 0x1f {
                            v.wrapping_neg() as u32
                        } else {
                            !v as u32
                        }
                        .into(),
                    );
                } else if op == 0x23 {
                    let v = pop(&mut stack)?;
                    stack.push(v.wrapping_add(uleb(expr, &mut at).ok_or_else(bad)?) & 0xffff_ffff);
                } else {
                    let b = pop(&mut stack)? as u32;
                    let a = pop(&mut stack)? as u32;
                    let r = match op {
                        0x1a => a & b,
                        0x1b => (a as i32).checked_div(b as i32).ok_or_else(bad)? as u32,
                        0x1c => a.wrapping_sub(b),
                        0x1d => a.checked_rem(b).ok_or_else(bad)?,
                        0x1e => a.wrapping_mul(b),
                        0x21 => a | b,
                        0x22 => a.wrapping_add(b),
                        0x24 => a.checked_shl(b).unwrap_or(0),
                        0x25 => a.checked_shr(b).unwrap_or(0),
                        0x26 => ((a as i32) >> b.min(31)) as u32,
                        0x27 => a ^ b,
                        _ => return Err(bad()),
                    };
                    stack.push(u64::from(r));
                }
            }
            0x28 => {
                let offset = i16::from_le_bytes(fixed(expr, &mut at).ok_or_else(bad)?);
                if pop(&mut stack)? != 0 {
                    at = (at as i64 + i64::from(offset)) as usize;
                }
            }
            0x29..=0x2e => {
                let b = pop(&mut stack)? as i32;
                let a = pop(&mut stack)? as i32;
                let r = match op {
                    0x29 => a == b,
                    0x2a => a >= b,
                    0x2b => a > b,
                    0x2c => a <= b,
                    0x2d => a < b,
                    _ => a != b,
                };
                stack.push(u64::from(r));
            }
            0x2f => {
                let offset = i16::from_le_bytes(fixed(expr, &mut at).ok_or_else(bad)?);
                at = (at as i64 + i64::from(offset)) as usize;
            }
            0x30..=0x4f => stack.push(u64::from(op - 0x30)),
            0x50..=0x6f => register = Some(u16::from(op - 0x50)),
            0x70..=0x8f => {
                let offset = sleb(expr, &mut at).ok_or_else(bad)?;
                let r = ctx
                    .regs
                    .get(usize::from(op - 0x70))
                    .copied()
                    .ok_or_else(bad)?;
                stack.push(u64::from(r.wrapping_add(offset as u32)));
            }
            0x90 => register = Some(uleb(expr, &mut at).ok_or_else(bad)? as u16),
            0x91 => {
                let offset = sleb(expr, &mut at).ok_or_else(bad)?;
                let base = ctx
                    .frame_base
                    .ok_or("la base de la fonction est inconnue")?;
                stack.push(u64::from(base.wrapping_add(offset as u32)));
            }
            0x92 => {
                let r = uleb(expr, &mut at).ok_or_else(bad)? as usize;
                let offset = sleb(expr, &mut at).ok_or_else(bad)?;
                let v = ctx.regs.get(r).copied().ok_or_else(bad)?;
                stack.push(u64::from(v.wrapping_add(offset as u32)));
            }
            0x93 => {
                let size = uleb(expr, &mut at).ok_or_else(bad)? as u32;
                let place = if let Some(r) = register.take() {
                    Place::Register(r)
                } else if let Some(bytes) = implicit.take() {
                    Place::Computed(bytes)
                } else if computed {
                    computed = false;
                    Place::Computed(pop(&mut stack)?.to_le_bytes()[..size.min(8) as usize].to_vec())
                } else if let Some(a) = stack.pop() {
                    Place::Address(a as u32)
                } else {
                    Place::OptimizedOut
                };
                pieces.push((place, size));
            }
            0x94 => {
                let size = fixed::<1>(expr, &mut at).ok_or_else(bad)?[0] as usize;
                let a = pop(&mut stack)? as u32;
                let bytes = ctx
                    .memory
                    .read(a, size)
                    .ok_or_else(|| format!("mémoire illisible à 0x{a:08x}"))?;
                let mut padded = [0u8; 8];
                padded[..size.min(8)].copy_from_slice(&bytes[..size.min(8)]);
                stack.push(u64::from_le_bytes(padded));
            }
            0x96 => {}
            0x9c => stack.push(u64::from(ctx.cfa.ok_or("la CFA est inconnue")?)),
            0x9e => {
                let len = uleb(expr, &mut at).ok_or_else(bad)? as usize;
                implicit = Some(expr.get(at..at + len).ok_or_else(bad)?.to_vec());
                at += len;
            }
            0x9f => computed = true,
            // `DW_OP_entry_value` (valeur du paramètre à l'entrée de la fonction) : inconnue ici.
            0xa3 | 0xf3 => return Ok(Place::OptimizedOut),
            other => return Err(format!("opération DWARF 0x{other:02x} non gérée")),
        }
    }
    if !pieces.is_empty() {
        return Ok(Place::Pieces(pieces));
    }
    if let Some(r) = register {
        return Ok(Place::Register(r));
    }
    if let Some(bytes) = implicit {
        return Ok(Place::Computed(bytes));
    }
    if computed {
        let v = stack.pop().ok_or_else(bad)?;
        return Ok(Place::Computed(v.to_le_bytes()[..4].to_vec()));
    }
    match stack.pop() {
        Some(a) => Ok(Place::Address(a as u32)),
        None => Ok(Place::OptimizedOut),
    }
}

// ---- valeurs ----------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ValueKind {
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    Char(i64),
    Pointer {
        address: u32,
        /// Pour `char *` : le début de la chaîne pointée.
        text: Option<String>,
        /// Le type pointé, pour déréférencer.
        target: Option<TypeId>,
    },
    Enum {
        value: i64,
        name: Option<String>,
    },
    Struct(Vec<(String, Value)>),
    /// Une énumération Rust : la variante active et ses champs.
    Variant {
        name: String,
        fields: Vec<(String, Value)>,
    },
    Array {
        items: Vec<Value>,
        /// Nombre total d'éléments (peut dépasser ceux que l'on a lus).
        total: u32,
    },
    Function(u32),
    OptimizedOut,
    Unavailable(String),
}

/// Une valeur lue : son type, son contenu, et l'endroit d'où elle vient (pour la modifier ou la surveiller).
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    pub ty: Option<TypeId>,
    pub type_name: String,
    pub kind: ValueKind,
    pub place: Option<Place>,
    pub size: u32,
}

/// Au plus ce nombre d'éléments par tableau, de niveaux de structures imbriquées.
const MAX_ITEMS: u32 = 64;
const MAX_DEPTH: u32 = 6;

impl Value {
    /// La valeur sur une ligne, comme l'écrirait un débogueur.
    pub fn summary(&self) -> String {
        match &self.kind {
            ValueKind::Int(v) => v.to_string(),
            ValueKind::UInt(v) => v.to_string(),
            ValueKind::Float(v) => format_float(*v),
            ValueKind::Bool(v) => v.to_string(),
            ValueKind::Char(v) => {
                let c = u8::try_from(*v & 0xff)
                    .ok()
                    .filter(|b| (0x20..0x7f).contains(b));
                match c {
                    Some(b) => format!("{v} '{}'", b as char),
                    None => v.to_string(),
                }
            }
            ValueKind::Pointer { address, text, .. } => match text {
                Some(s) => format!("0x{address:08x} \"{s}\""),
                None => format!("0x{address:08x}"),
            },
            ValueKind::Enum { value, name } => match name {
                Some(n) => n.clone(),
                None => value.to_string(),
            },
            ValueKind::Struct(fields) => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(n, v)| format!("{n} = {}", v.summary()))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            ValueKind::Variant { name, fields } => {
                if fields.is_empty() {
                    name.clone()
                } else if fields.iter().all(|(n, _)| n.starts_with("__")) {
                    // Une variante « tuple » : `Some(4)`.
                    let parts: Vec<String> = fields.iter().map(|(_, v)| v.summary()).collect();
                    format!("{name}({})", parts.join(", "))
                } else {
                    let parts: Vec<String> = fields
                        .iter()
                        .map(|(n, v)| format!("{n} = {}", v.summary()))
                        .collect();
                    format!("{name} {{{}}}", parts.join(", "))
                }
            }
            ValueKind::Array { items, total } => {
                let parts: Vec<String> = items.iter().map(Value::summary).collect();
                let more = if *total as usize > items.len() {
                    ", …"
                } else {
                    ""
                };
                format!("{{{}{more}}}", parts.join(", "))
            }
            ValueKind::Function(a) => format!("{{{}}} 0x{a:08x}", self.type_name),
            ValueKind::OptimizedOut => "<optimisé>".to_owned(),
            ValueKind::Unavailable(why) => format!("<{why}>"),
        }
    }

    /// Une description JSON : type, résumé, adresse, et les champs ou éléments.
    pub fn to_json(&self) -> Json {
        let mut out = Json::object()
            .with("type", self.type_name.as_str())
            .with("value", self.summary())
            .with("size", self.size);
        if let Some(Place::Address(a)) = &self.place {
            out.set("address", *a);
        }
        if let Some(Place::Register(r)) = &self.place {
            out.set("register", *r);
        }
        match &self.kind {
            ValueKind::Struct(fields) | ValueKind::Variant { fields, .. } => {
                let children: Vec<Json> = fields
                    .iter()
                    .map(|(n, v)| v.to_json().with("name", n.as_str()))
                    .collect();
                out.set("children", children);
            }
            ValueKind::Array { items, total } => {
                let children: Vec<Json> = items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| v.to_json().with("name", format!("[{i}]")))
                    .collect();
                out.set("children", children);
                out.set("count", *total);
            }
            ValueKind::Pointer {
                address,
                target: Some(_),
                ..
            } if *address != 0 => {
                out.set("pointer", *address);
            }
            _ => {}
        }
        out
    }
}

fn format_float(v: f64) -> String {
    if v.is_nan() {
        "nan".to_owned()
    } else if v.is_infinite() {
        if v > 0.0 {
            "inf".to_owned()
        } else {
            "-inf".to_owned()
        }
    } else if v == v.trunc() && v.abs() < 1e15 {
        format!("{v:.1}")
    } else {
        // 9 chiffres significatifs, sans zéros inutiles.
        let s = format!("{v:.9e}");
        let parsed: f64 = s.parse().unwrap_or(v);
        format!("{parsed}")
    }
}

fn register_bytes(ctx: &Context<'_>, reg: u16, len: u32) -> Option<Vec<u8>> {
    let words: Vec<u32> = match reg {
        0..=15 => vec![ctx.regs[usize::from(reg)]],
        64..=95 => vec![ctx.fregs[usize::from(reg - 64)]],
        256..=287 => {
            let i = usize::from(reg - 256) * 2;
            vec![*ctx.fregs.get(i)?, *ctx.fregs.get(i + 1)?]
        }
        _ => return None,
    };
    let mut bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    bytes.truncate(len as usize);
    Some(bytes)
}

/// Les octets d'une valeur de `len` octets à `place`.
fn bytes_at(place: &Place, len: u32, ctx: &mut Context<'_>) -> Result<Vec<u8>, String> {
    match place {
        Place::Address(a) => ctx
            .memory
            .read(*a, len as usize)
            .ok_or_else(|| format!("mémoire illisible à 0x{a:08x}")),
        Place::Register(r) => {
            register_bytes(ctx, *r, len).ok_or_else(|| "registre inconnu".to_owned())
        }
        Place::Computed(v) => {
            let mut v = v.clone();
            v.resize(len as usize, 0);
            Ok(v)
        }
        Place::Pieces(pieces) => {
            let mut out = Vec::new();
            for (p, size) in pieces {
                out.extend(bytes_at(p, *size, ctx).unwrap_or_else(|_| vec![0; *size as usize]));
            }
            out.resize(len as usize, 0);
            Ok(out)
        }
        Place::OptimizedOut => Err("optimisé".to_owned()),
    }
}

fn le(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .take(8)
        .enumerate()
        .fold(0, |acc, (i, b)| acc | (u64::from(*b) << (8 * i)))
}

fn sign_extend(v: u64, bits: u32) -> i64 {
    if bits == 0 || bits >= 64 {
        v as i64
    } else {
        let shift = 64 - bits;
        ((v << shift) as i64) >> shift
    }
}

/// Lit la valeur de type `ty` à `place`.
pub fn read_value(
    info: &DebugInfo,
    ty: Option<TypeId>,
    place: Place,
    ctx: &mut Context<'_>,
) -> Value {
    read(info, ty, place, ctx, 0)
}

fn read(
    info: &DebugInfo,
    ty: Option<TypeId>,
    place: Place,
    ctx: &mut Context<'_>,
    depth: u32,
) -> Value {
    let type_name = info.type_name(ty);
    let Some(ty) = ty else {
        return Value {
            ty: None,
            type_name,
            kind: ValueKind::Unavailable("type void".into()),
            place: Some(place),
            size: 0,
        };
    };
    let size = info.size_of(ty);
    let value = |kind: ValueKind, place: Place| Value {
        ty: Some(ty),
        type_name: type_name.clone(),
        kind,
        place: Some(place),
        size,
    };
    if place == Place::OptimizedOut {
        return value(ValueKind::OptimizedOut, place);
    }
    let stripped = info.strip(ty);
    let t = info.ty(stripped);
    match &t.kind {
        TypeKind::Base(enc) => match bytes_at(&place, size, ctx) {
            Err(e) => value(ValueKind::Unavailable(e), place),
            Ok(bytes) => {
                let raw = le(&bytes);
                let bits = size * 8;
                let kind = match enc {
                    Encoding::Float if size == 4 => {
                        ValueKind::Float(f64::from(f32::from_bits(raw as u32)))
                    }
                    Encoding::Float => ValueKind::Float(f64::from_bits(raw)),
                    Encoding::Bool => ValueKind::Bool(raw != 0),
                    Encoding::Signed => ValueKind::Int(sign_extend(raw, bits)),
                    Encoding::SignedChar => ValueKind::Char(sign_extend(raw, bits)),
                    Encoding::UnsignedChar => ValueKind::Char(raw as i64),
                    Encoding::Unsigned => ValueKind::UInt(raw),
                };
                value(kind, place)
            }
        },
        TypeKind::Enum { variants, under } => match bytes_at(&place, size.max(1), ctx) {
            Err(e) => value(ValueKind::Unavailable(e), place),
            Ok(bytes) => {
                let raw = le(&bytes);
                let signed_under = under.is_some_and(|u| {
                    matches!(
                        info.ty(info.strip(u)).kind,
                        TypeKind::Base(Encoding::Signed | Encoding::SignedChar)
                    )
                });
                let v = if signed_under {
                    sign_extend(raw, size * 8)
                } else {
                    raw as i64
                };
                let name = variants
                    .iter()
                    .find(|(_, x)| *x == v)
                    .map(|(n, _)| n.clone());
                value(ValueKind::Enum { value: v, name }, place)
            }
        },
        TypeKind::Pointer(target) => match bytes_at(&place, 4, ctx) {
            Err(e) => value(ValueKind::Unavailable(e), place),
            Ok(bytes) => {
                let address = le(&bytes) as u32;
                let mut text = None;
                if let Some(t) = target {
                    let pointee = info.strip(*t);
                    if matches!(
                        info.ty(pointee).kind,
                        TypeKind::Base(Encoding::SignedChar | Encoding::UnsignedChar)
                    ) && info.size_of(pointee) == 1
                        && address != 0
                    {
                        text = read_c_string(ctx, address);
                    }
                }
                value(
                    ValueKind::Pointer {
                        address,
                        text,
                        target: *target,
                    },
                    place,
                )
            }
        },
        TypeKind::Reference(target) => match bytes_at(&place, 4, ctx) {
            Err(e) => value(ValueKind::Unavailable(e), place),
            Ok(bytes) => {
                // Une référence se lit comme ce qu'elle désigne ; l'emplacement est celui de l'objet.
                let address = le(&bytes) as u32;
                if address == 0 {
                    return value(ValueKind::Unavailable("référence nulle".into()), place);
                }
                let mut v = read(info, *target, Place::Address(address), ctx, depth);
                v.ty = Some(ty);
                v.type_name = type_name.clone();
                v
            }
        },
        TypeKind::Struct {
            fields, variants, ..
        } => {
            if depth >= MAX_DEPTH {
                return value(ValueKind::Unavailable("…".into()), place);
            }
            if let (Some(part), Place::Address(base)) = (variants, &place) {
                // Une énumération Rust : l'étiquette dit quelle variante est active.
                let tag = bytes_at(
                    &Place::Address(base.wrapping_add(part.discr_offset)),
                    part.discr_size,
                    ctx,
                )
                .map(|b| le(&b) as i64)
                .unwrap_or(0);
                let active = part
                    .variants
                    .iter()
                    .find(|v| v.discr == Some(tag))
                    .or_else(|| part.variants.iter().find(|v| v.discr.is_none()));
                return match active {
                    Some(v) => {
                        let inner = read(
                            info,
                            Some(v.ty),
                            Place::Address(base.wrapping_add(v.offset)),
                            ctx,
                            depth + 1,
                        );
                        let fields = match inner.kind {
                            ValueKind::Struct(f) => f,
                            _ => Vec::new(),
                        };
                        value(
                            ValueKind::Variant {
                                name: v.name.clone(),
                                fields,
                            },
                            place,
                        )
                    }
                    None => value(ValueKind::Unavailable("variante inconnue".into()), place),
                };
            }
            let Place::Address(base) = place else {
                // Un agrégat dans des registres : on lit chaque champ dans les octets rassemblés.
                let all = bytes_at(&place, size, ctx).unwrap_or_default();
                let mut out = Vec::new();
                for f in fields {
                    let fsize = info.size_of(f.ty);
                    let part = all
                        .get(f.offset as usize..(f.offset + fsize) as usize)
                        .map(<[u8]>::to_vec)
                        .unwrap_or_default();
                    out.push((
                        field_label(f),
                        read(info, Some(f.ty), Place::Computed(part), ctx, depth + 1),
                    ));
                }
                return value(ValueKind::Struct(out), place);
            };
            let mut out = Vec::new();
            for f in fields {
                let mut v = read(
                    info,
                    Some(f.ty),
                    Place::Address(base.wrapping_add(f.offset)),
                    ctx,
                    depth + 1,
                );
                if let Some((bit_size, bit_offset)) = f.bits {
                    // Champ de bits : on relit le mot qui le contient.
                    if let Ok(bytes) =
                        bytes_at(&Place::Address(base.wrapping_add(f.offset)), 4, ctx)
                    {
                        let raw = (le(&bytes) >> bit_offset) & ((1u64 << bit_size) - 1);
                        v.kind = match v.kind {
                            ValueKind::Int(_) | ValueKind::Char(_) => {
                                ValueKind::Int(sign_extend(raw, u32::from(bit_size)))
                            }
                            ValueKind::Bool(_) => ValueKind::Bool(raw != 0),
                            _ => ValueKind::UInt(raw),
                        };
                    }
                }
                out.push((field_label(f), v));
            }
            value(ValueKind::Struct(out), place)
        }
        TypeKind::Array { elem, counts } => {
            if depth >= MAX_DEPTH {
                return value(ValueKind::Unavailable("…".into()), place);
            }
            let total: u32 = counts.iter().map(|c| c.unwrap_or(0)).product();
            let esize = info.size_of(*elem);
            let Place::Address(base) = place else {
                return value(ValueKind::Unavailable("tableau hors mémoire".into()), place);
            };
            // Un tableau de caractères : on le montre comme du texte (par dessus les éléments).
            let mut items = Vec::new();
            for i in 0..total.min(MAX_ITEMS) {
                items.push(read(
                    info,
                    Some(*elem),
                    Place::Address(base.wrapping_add(i * esize)),
                    ctx,
                    depth + 1,
                ));
            }
            value(ValueKind::Array { items, total }, place)
        }
        TypeKind::Function { .. } => {
            let a = if let Place::Address(a) = place { a } else { 0 };
            value(ValueKind::Function(a), place)
        }
        _ => value(ValueKind::Unavailable("type inconnu".into()), place),
    }
}

fn read_c_string(ctx: &mut Context<'_>, address: u32) -> Option<String> {
    let mut out = Vec::new();
    for chunk in 0..4u32 {
        let bytes = ctx
            .memory
            .read(address.wrapping_add(chunk * 16), 16)
            .or_else(|| ctx.memory.read(address.wrapping_add(chunk * 16), 1))?;
        for b in bytes {
            if b == 0 {
                return Some(escape(&out));
            }
            out.push(b);
        }
    }
    let mut s = escape(&out);
    s.push('…');
    Some(s)
}

fn escape(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        match b {
            b'\n' => s.push_str("\\n"),
            b'\t' => s.push_str("\\t"),
            b'"' => s.push_str("\\\""),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7e => s.push(b as char),
            _ => s.push_str(&format!("\\x{b:02x}")),
        }
    }
    s
}

/// Le nom d'un champ à l'affichage : une classe de base s'écrit `<Base>`.
fn field_label(f: &super::Field) -> String {
    if f.base {
        format!("<{}>", f.name)
    } else {
        f.name.clone()
    }
}
