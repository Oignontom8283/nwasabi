//! Un petit JSON (analyse et écriture) sans dépendance, ainsi que base64 et hexadécimal.
//!
//! Il sert au protocole de session (voir `session`) : le même format pour l'outil en ligne de
//! commande, le serveur et la page web. Les objets gardent l'ordre des clés, et l'écriture est
//! déterministe, pour que les sorties puissent être comparées telles quelles.

use std::fmt;

/// Une valeur JSON. Les entiers sont gardés à part des flottants pour ne rien perdre au-delà de 2^53.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn object() -> Self {
        Self::Object(Vec::new())
    }

    /// Ajoute (ou remplace) une clé d'un objet ; sans effet sur une autre valeur.
    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.set(key, value);
        self
    }

    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        if let Self::Object(entries) = self {
            let value = value.into();
            match entries.iter_mut().find(|(k, _)| k == key) {
                Some((_, v)) => *v = value,
                None => entries.push((key.to_owned(), value)),
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Float(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(i) => Some(*i as f64),
            Self::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Un entier non signé de 32 bits, écrit en nombre ou en chaîne (`"0x20000000"`).
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Self::Str(s) => {
                let s = s.trim();
                match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => s.parse().ok(),
                }
            }
            other => other.as_i64().and_then(|i| u32::try_from(i).ok()),
        }
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}
impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Self::Int(i64::try_from(v).unwrap_or(i64::MAX))
    }
}
impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Self::Int(i64::from(v))
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int(i64::from(v))
    }
}
impl From<usize> for Value {
    fn from(v: usize) -> Self {
        Self::Int(i64::try_from(v).unwrap_or(i64::MAX))
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Str(v.to_owned())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Str(v)
    }
}
impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Self::Array(v.into_iter().map(Into::into).collect())
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        v.map_or(Self::Null, Into::into)
    }
}

// ---- écriture ----------------------------------------------------------------------------------

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

impl fmt::Display for Value {
    /// JSON compact sur une seule ligne.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        self.write(&mut out);
        f.write_str(&out)
    }
}

impl Value {
    fn write(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Int(i) => out.push_str(&i.to_string()),
            Self::Float(x) if x.is_finite() => {
                let text = format!("{x}");
                out.push_str(&text);
                if !text.contains(['.', 'e', 'E']) {
                    out.push_str(".0");
                }
            }
            Self::Float(_) => out.push_str("null"),
            Self::Str(s) => write_string(out, s),
            Self::Array(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Self::Object(entries) => {
                out.push('{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(out, k);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

