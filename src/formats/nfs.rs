//! `.nfs` — Nwasabi File System : les fichiers de la calculatrice (scripts Python, réglages…).
//!
//! ```json
//! { "nfs": 1, "capacity": 43008, "files": [ { "name": "main.py", "content": "…" } ] }
//! ```
//!
//! Les fichiers sont dans l'ordre du tampon de la calculatrice. Le format décrit des fichiers, pas
//! l'image brute du tampon : il se recharge donc sur n'importe quel modèle ou firmware.

use super::{base64, field, number, text};
use crate::json::{self, Value, ValueExt};
use crate::storage::Fs;
use crate::storage::records;

/// Version du format.
pub const VERSION: u64 = 1;

/// Enregistre les fichiers de `fs`.
pub fn export(fs: &mut Fs<'_>) -> Result<Value, String> {
    let listing = fs.list().map_err(|e| e.to_string())?;
    if let Some(offset) = listing.corrupt_at {
        return Err(format!(
            "système de fichiers abîmé à l'octet {offset} : rien n'est exporté"
        ));
    }
    let stats = fs.stats().map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for info in &listing.files {
        let body = fs.read(&info.name).map_err(|e| e.to_string())?;
        files.push(
            Value::object()
                .with("name", info.name.as_str())
                .with("content", json::base64_encode(&body)),
        );
    }
    Ok(Value::object()
        .with("nfs", VERSION)
        .with("capacity", stats.total)
        .with("files", files))
}

/// Les fichiers d'un document, vérifiés.
pub fn parse(document: &Value) -> Result<Vec<(String, Vec<u8>)>, String> {
    let version = number(document, "nfs")
        .map_err(|_| "ce n'est pas un fichier .nfs (champ « nfs » manquant)".to_owned())?;
    if version != VERSION {
        return Err(format!(
            "version {version} non prise en charge (seule {VERSION} l'est)"
        ));
    }
    let list = field(document, "files")?
        .as_array()
        .ok_or("« files » : une liste est attendue")?;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for (i, entry) in list.iter().enumerate() {
        let at = |m: String| format!("files[{i}] : {m}");
        let name = text(entry, "name").map_err(at)?.to_owned();
        let body = base64(entry, "content").map_err(at)?;
        records::check_name(&name).map_err(|e| at(e.to_string()))?;
        if body.len() > 65_535 {
            return Err(at("fichier trop grand pour un enregistrement".into()));
        }
        if files.iter().any(|(n, _)| *n == name) {
            return Err(at(format!("nom en double : {name}")));
        }
        files.push((name, body));
    }
    Ok(files)
}

/// Charge les fichiers du document dans `fs`. Avec `replace`, tous les fichiers existants sont d'abord
/// supprimés ; sinon, ceux du même nom sont remplacés et les autres gardés. Si l'opération échoue (plus de
/// place…), `fs` est remis dans l'état où il était. Rend le nombre de fichiers chargés.
pub fn import(fs: &mut Fs<'_>, document: &Value, replace: bool) -> Result<usize, String> {
    let files = parse(document)?;
    let before = fs.raw().map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        if replace {
            let listing = fs.list().map_err(|e| e.to_string())?;
            for info in listing.files {
                fs.remove(&info.name).map_err(|e| e.to_string())?;
            }
        }
        for (name, body) in &files {
            fs.write(name, body).map_err(|e| format!("{name} : {e}"))?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        fs.restore_raw(&before).map_err(|e| e.to_string())?;
        return Err(error);
    }
    Ok(files.len())
}
