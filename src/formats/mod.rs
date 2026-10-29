//! Formats de fichiers de nwasabi : les fichiers de la calculatrice (`.nfs`) et l'état d'une
//! émulation (`.nds`), les octets à placer en flash (`.nfd`). Les spécifications publiques sont dans `docs/reference/formats/`. (Le mapping des touches,
//! `.nmk`, est dans `keymap`.)

pub mod nds;
pub mod nfd;
pub mod nfs;
pub mod nrp;

use crate::json::Value;

/// Un champ d'un document, avec une erreur lisible s'il manque.
pub(crate) fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, String> {
    value
        .get(name)
        .ok_or_else(|| format!("champ « {name} » manquant"))
}

pub(crate) fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| format!("« {name} » : un texte est attendu"))
}

pub(crate) fn base64(value: &Value, name: &str) -> Result<Vec<u8>, String> {
    crate::json::base64_decode(text(value, name)?)
        .ok_or_else(|| format!("« {name} » : contenu invalide (base64 attendu)"))
}

pub(crate) fn number(value: &Value, name: &str) -> Result<u64, String> {
    field(value, name)?
        .as_u64()
        .ok_or_else(|| format!("« {name} » : un entier positif est attendu"))
}

pub(crate) fn number32(value: &Value, name: &str) -> Result<u32, String> {
    u32::try_from(number(value, name)?).map_err(|_| format!("« {name} » : valeur trop grande"))
}

pub(crate) fn boolean(value: &Value, name: &str) -> Result<bool, String> {
    field(value, name)?
        .as_bool()
        .ok_or_else(|| format!("« {name} » : vrai ou faux est attendu"))
}

#[cfg(test)]
mod tests;
