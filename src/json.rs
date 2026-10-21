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

// ---- analyse -----------------------------------------------------------------------------------

/// Erreur d'analyse, avec la position (en octets) du problème.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub position: usize,
    pub message: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "JSON invalide à l'octet {} : {}",
            self.position, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// Profondeur maximale d'imbrication acceptée (protège la pile contre des entrées hostiles).
const MAX_DEPTH: usize = 64;

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn err<T>(&self, message: &'static str) -> Result<T, ParseError> {
        Err(ParseError {
            position: self.pos,
            message,
        })
    }

    fn skip_space(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        self.skip_space();
        if self.bytes.get(self.pos) == Some(&byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, ParseError> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            self.err("valeur inconnue")
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth > MAX_DEPTH {
            return self.err("trop d'imbrications");
        }
        self.skip_space();
        match self.bytes.get(self.pos) {
            None => self.err("fin inattendue"),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                if self.eat(b']') {
                    return Ok(Value::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    if self.eat(b',') {
                        continue;
                    }
                    if self.eat(b']') {
                        return Ok(Value::Array(items));
                    }
                    return self.err("« , » ou « ] » attendu");
                }
            }
            Some(b'{') => {
                self.pos += 1;
                let mut entries: Vec<(String, Value)> = Vec::new();
                if self.eat(b'}') {
                    return Ok(Value::Object(entries));
                }
                loop {
                    self.skip_space();
                    if self.bytes.get(self.pos) != Some(&b'"') {
                        return self.err("clé attendue");
                    }
                    let key = self.string()?;
                    if !self.eat(b':') {
                        return self.err("« : » attendu");
                    }
                    let value = self.value(depth + 1)?;
                    match entries.iter_mut().find(|(k, _)| *k == key) {
                        Some((_, v)) => *v = value,
                        None => entries.push((key, value)),
                    }
                    if self.eat(b',') {
                        continue;
                    }
                    if self.eat(b'}') {
                        return Ok(Value::Object(entries));
                    }
                    return self.err("« , » ou « } » attendu");
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => self.err("caractère inattendu"),
        }
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        while matches!(
            self.bytes.get(self.pos),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap_or("");
        if let Ok(i) = text.parse::<i64>() {
            return Ok(Value::Int(i));
        }
        match text.parse::<f64>() {
            Ok(f) if f.is_finite() => Ok(Value::Float(f)),
            _ => {
                self.pos = start;
                self.err("nombre invalide")
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let digits = self.bytes.get(self.pos..self.pos + 4);
        let value = digits
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u32::from_str_radix(d, 16).ok());
        match value {
            Some(v) => {
                self.pos += 4;
                Ok(v)
            }
            None => self.err("séquence \\u invalide"),
        }
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1; // guillemet ouvrant
        let mut out = Vec::new();
        loop {
            let Some(&b) = self.bytes.get(self.pos) else {
                return self.err("chaîne non terminée");
            };
            self.pos += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let Some(&e) = self.bytes.get(self.pos) else {
                        return self.err("échappement incomplet");
                    };
                    self.pos += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'u' => {
                            let mut code = self.hex4()?;
                            if (0xd800..0xdc00).contains(&code)
                                && self.bytes[self.pos..].starts_with(b"\\u")
                            {
                                self.pos += 2;
                                let low = self.hex4()?;
                                code = 0x10000 + ((code - 0xd800) << 10) + low.wrapping_sub(0xdc00);
                            }
                            char::from_u32(code).unwrap_or('\u{fffd}')
                        }
                        _ => return self.err("échappement inconnu"),
                    };
                    let mut buf = [0; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                0..=0x1f => return self.err("caractère de contrôle dans une chaîne"),
                b => out.push(b),
            }
        }
        String::from_utf8(out).or_else(|_| self.err("UTF-8 invalide"))
    }
}

/// Analyse un document JSON complet.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    let value = p.value(0)?;
    p.skip_space();
    if p.pos != p.bytes.len() {
        return p.err("texte après la valeur");
    }
    Ok(value)
}

// ---- base64 et hexadécimal ----------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Décode du base64 (avec ou sans remplissage, espaces ignorés).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for c in text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
    {
        let v = B64.iter().position(|&x| x == c)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    (bits < 6).then_some(out)
}

pub fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hex_decode(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if digits.len() % 2 != 0 {
        return None;
    }
    digits
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).ok()?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_writes_back_the_same_compact_text() {
        let text = r#"{"a":1,"b":[true,false,null],"c":"x\ny","d":-2.5,"e":{}}"#;
        let v = parse(text).unwrap();
        assert_eq!(v.to_string(), text);
        assert_eq!(v.get("a").and_then(Value::as_i64), Some(1));
        assert_eq!(v.get("d").and_then(Value::as_f64), Some(-2.5));
    }

    #[test]
    fn keeps_key_order_and_the_last_duplicate() {
        let v = parse(r#"{"z":1,"a":2,"z":3}"#).unwrap();
        assert_eq!(v.to_string(), r#"{"z":3,"a":2}"#);
    }

    #[test]
    fn strings_handle_escapes_unicode_and_surrogates() {
        let v = parse(r#""éé😀\t\"""#).unwrap();
        assert_eq!(v.as_str(), Some("éé😀\t\""));
        assert_eq!(Value::from("a\u{1}b").to_string(), r#""a\u0001b""#);
    }

