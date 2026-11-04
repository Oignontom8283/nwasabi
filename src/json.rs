//! JSON : `serde_json` pour l'analyse et l'écriture, plus quelques aides pour construire et lire
//! les requêtes et les réponses du protocole de session, ainsi que base64 et hexadécimal.
//!
//! Les objets sont écrits avec leurs clés triées : la sortie est déterministe, pour que les
//! résultats puissent être comparés tels quels.

use base64::Engine as _;
use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig, general_purpose};

pub use serde_json::Value;

/// Erreur d'analyse (position et description de `serde_json`).
pub type ParseError = serde_json::Error;

/// Analyse un document JSON complet.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    serde_json::from_str(text)
}

/// Aides pour manipuler les valeurs du protocole.
pub trait ValueExt {
    /// Un objet vide.
    fn object() -> Value;
    /// Ajoute (ou remplace) une clé d'un objet et rend l'objet ; sans effet sur une autre valeur.
    fn with(self, key: &str, value: impl Into<Value>) -> Value;
    /// Ajoute (ou remplace) une clé d'un objet ; sans effet sur une autre valeur.
    fn set(&mut self, key: &str, value: impl Into<Value>);
    /// Un entier non signé de 32 bits, écrit en nombre ou en chaîne (`"0x20000000"`).
    fn as_u32(&self) -> Option<u32>;
}

impl ValueExt for Value {
    fn object() -> Value {
        Value::Object(serde_json::Map::new())
    }

    fn with(mut self, key: &str, value: impl Into<Value>) -> Value {
        self.set(key, value);
        self
    }

    fn set(&mut self, key: &str, value: impl Into<Value>) {
        if let Value::Object(map) = self {
            map.insert(key.to_owned(), value.into());
        }
    }

    fn as_u32(&self) -> Option<u32> {
        match self {
            Value::String(s) => {
                let s = s.trim();
                match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => s.parse().ok(),
                }
            }
            other => other.as_u64().and_then(|n| u32::try_from(n).ok()),
        }
    }
}

// ---- base64 et hexadécimal ----------------------------------------------------------------------

/// Le décodeur de base64 : sans remplissage obligatoire, et des bits de fin non nuls acceptés (comme les anciens fichiers).
const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

pub fn base64_encode(data: &[u8]) -> String {
    general_purpose::STANDARD.encode(data)
}

/// Décode du base64 (avec ou sans remplissage, espaces ignorés).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    LENIENT.decode(digits).ok()
}

pub fn hex_encode(data: &[u8]) -> String {
    hex::encode(data)
}

/// Décode de l'hexadécimal (majuscules ou minuscules, espaces ignorés).
pub fn hex_decode(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    hex::decode(digits).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_are_built_and_written_deterministically() {
        let v = Value::object()
            .with("b", 1)
            .with("a", "x\ny")
            .with("c", vec![true]);
        assert_eq!(v.to_string(), r#"{"a":"x\ny","b":1,"c":[true]}"#);
        assert_eq!(v.get("b").and_then(Value::as_i64), Some(1));
        let replaced = v.with("b", 2);
        assert_eq!(replaced.get("b").and_then(Value::as_i64), Some(2));
    }

    #[test]
    fn addresses_can_be_numbers_or_hex_strings() {
        assert_eq!(Value::from(4096).as_u32(), Some(4096));
        assert_eq!(Value::from("0x20000000").as_u32(), Some(0x2000_0000));
        assert_eq!(Value::from("12").as_u32(), Some(12));
        assert_eq!(Value::from(-1).as_u32(), None);
        assert_eq!(Value::from(1u64 << 40).as_u32(), None);
    }

    #[test]
    fn malformed_documents_are_rejected() {
        for bad in [
            "",
            "{",
            "[1,]",
            r#"{"a" 1}"#,
            "tru",
            "1 2",
            r#""abc"#,
            "{1:2}",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        let deep = "[".repeat(1000) + &"]".repeat(1000);
        assert!(parse(&deep).is_err(), "l'imbrication est limitée");
    }

    #[test]
    fn base64_matches_the_standard_vectors() {
        for (plain, coded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(plain.as_bytes()), coded);
            assert_eq!(base64_decode(coded).unwrap(), plain.as_bytes());
        }
        assert_eq!(base64_decode("Zm9v\nYmFy").unwrap(), b"foobar");
        assert_eq!(base64_decode("Zm9vYmE").unwrap(), b"fooba");
        assert!(base64_decode("Zm9v!").is_none());
        assert!(base64_decode("Z").is_none());
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(hex_encode(&[0, 0xab, 0xff]), "00abff");
        assert_eq!(hex_decode("00 AB ff").unwrap(), [0, 0xab, 0xff]);
        assert!(hex_decode("abc").is_none());
        assert!(hex_decode("zz").is_none());
        assert!(
            hex_decode("+a").is_none(),
            "un signe n'est pas un chiffre hexadécimal"
        );
    }
}
