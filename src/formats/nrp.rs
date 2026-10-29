//! `.nrp` — Nwasabi RePlay : l'enregistrement d'une session (touches, temps qui passe, exécutions), à rejouer plus tard.
//!
//! ```json
//! { "nrp": 1,
//!   "device": { "model": "n0110", "firmware": "24" },
//!   "app": { "name": "jeu.nwa", "crc32": "1e46165e" },
//!   "entries": [
//!     { "request": { "cmd": "key", "key": "ok", "action": "press" } },
//!     { "request": { "cmd": "run", "frames": 30 }, "expect": { "stop": "frame", "instructions": 1520301 } },
//!     { "request": { "cmd": "screen" }, "expect": { "crc32": "50862fc7" } } ] }
//! ```
//!
//! L'émulation est déterministe : les mêmes requêtes, rejouées sur un appareil neuf avec la même application, redonnent les mêmes
//! arrêts, le même nombre d'instructions et le même écran. Ce que l'on avait observé à l'enregistrement (`expect`) sert à vérifier
//! que c'est bien le cas : c'est un test de non-régression sans image de référence à garder.

use crate::checksum::crc32;
use crate::json::{self, Value, ValueExt};
use crate::session::Session;

/// Version du format.
pub const VERSION: u64 = 1;

/// Les commandes qui changent ce que l'application voit ou fait : seules elles sont rejouées.
const REPLAYED: &[&str] = &[
    "key",
    "advance",
    "run",
    "step",
    "launch",
    "speed",
    "cpu_set",
    "memory_write",
    "fs_write",
    "fs_remove",
    "fs_rename",
    "fs_import",
    "settings_set",
    "flash_load",
    "assign",
    "break",
    "unbreak",
    "break_line",
    "unbreak_line",
    "break_function",
    "watch",
    "unwatch",
    "step_source",
    "step_back",
    "reverse_continue",
    "history_goto",
    "history",
];

/// Les observations qui deviennent des vérifications.
const CHECKED: &[&str] = &["screen", "stdout"];

fn crc_text(bytes: &[u8]) -> String {
    format!("{:08x}", crc32(bytes))
}

/// Construit un enregistrement au fil des requêtes d'une session.
#[derive(Debug, Default)]
pub struct Recorder {
    device: Option<Value>,
    app: Option<Value>,
    entries: Vec<Value>,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Note une requête et sa réponse (à appeler après chaque requête de la session).
    pub fn note(&mut self, request: &Value, reply: &Value) {
        let Some(cmd) = request.get("cmd").and_then(Value::as_str) else {
            return;
        };
        if reply.get("ok") != Some(&Value::Bool(true)) {
            return;
        }
        match cmd {
            "load" | "new" => {
                let mut device = Value::object();
                for key in ["model", "firmware", "clock", "speed", "mhz"] {
                    if let Some(v) = request.get(key) {
                        device.set(key, v.clone());
                    }
                }
                // Un nouvel appareil : ce qui précède n'a plus d'effet.
                self.entries.clear();
                self.device = Some(device);
                if cmd == "load" {
                    let mut app = Value::object();
                    if let Some(nwa) = request
                        .get("nwa")
                        .and_then(Value::as_str)
                        .and_then(json::base64_decode)
                    {
                        app.set("crc32", crc_text(&nwa));
                    }
                    if let Some(data) = request
                        .get("data")
                        .and_then(Value::as_str)
                        .and_then(json::base64_decode)
                    {
                        app.set("data_crc32", crc_text(&data));
                    }
                    if request.get("nfd").is_some() {
                        app.set("flash", true);
                    }
                    self.app = Some(app);
                }
            }
            c if REPLAYED.contains(&c) => {
                let mut entry = Value::object().with("request", request.clone());
                let mut expect = Value::object();
                if let Some(kind) = reply.get("stop").and_then(|s| s.get("kind")) {
                    expect.set("stop", kind.clone());
                }
                if let Some(n) = reply.get("instructions") {
                    expect.set("instructions", n.clone());
                }
                if expect.as_object().is_some_and(|o| !o.is_empty()) {
                    entry.set("expect", expect);
                }
                self.entries.push(entry);
            }
            c if CHECKED.contains(&c) => {
                let mut expect = Value::object();
                if let Some(crc) = reply.get("crc32") {
                    expect.set("crc32", crc.clone());
                }
                if let Some(text) = reply.get("text").and_then(Value::as_str) {
                    expect.set("text_crc32", crc_text(text.as_bytes()));
                }
                if expect.as_object().is_some_and(|o| !o.is_empty()) {
                    // Sans les pixels : l'écran se compare par son CRC.
                    let mut r = request.clone();
                    if cmd == "screen" {
                        for key in ["rgba", "rgb565"] {
                            r.set(key, false);
                        }
                    }
                    self.entries
                        .push(Value::object().with("request", r).with("expect", expect));
                }
            }
            _ => {}
        }
    }

    /// Le document `.nrp`.
    pub fn finish(&self, app_name: Option<&str>) -> Value {
        let mut app = self.app.clone().unwrap_or_else(Value::object);
        if let Some(name) = app_name {
            app.set("name", name);
        }
        Value::object()
            .with("nrp", VERSION)
            .with(
                "created_by",
                format!("nwasabi {}", env!("CARGO_PKG_VERSION")),
            )
            .with("device", self.device.clone().unwrap_or_else(Value::object))
            .with("app", app)
            .with("entries", self.entries.clone())
    }
}

/// Ce que l'on fournit pour rejouer : l'application, et ce qui l'accompagnait à l'enregistrement.
pub struct Material<'a> {
    pub nwa: &'a [u8],
    pub data: Option<&'a [u8]>,
    /// Document `.nfd`.
    pub flash: Option<&'a Value>,
}

/// Une différence entre l'enregistrement et la rediffusion.
#[derive(Debug, Clone, PartialEq)]
pub struct Mismatch {
    /// Rang de l'entrée (à partir de 1).
    pub entry: usize,
    pub what: String,
    pub recorded: String,
    pub replayed: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub entries: usize,
    pub checks: usize,
    pub mismatches: Vec<Mismatch>,
    /// Les avertissements : application ou données différentes de l'enregistrement.
    pub warnings: Vec<String>,
}

/// Vérifie qu'un document est un `.nrp` lisible et rend ses entrées.
pub fn parse(document: &Value) -> Result<&Vec<Value>, String> {
    let version = document
        .get("nrp")
        .and_then(Value::as_u64)
        .ok_or("ce n'est pas un fichier .nrp (champ « nrp » manquant)")?;
    if version != VERSION {
        return Err(format!(
            "version {version} non prise en charge (seule {VERSION} l'est)"
        ));
    }
    let entries = document
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("« entries » : une liste est attendue")?;
    for (i, entry) in entries.iter().enumerate() {
        if entry
            .get("request")
            .and_then(|r| r.get("cmd"))
            .and_then(Value::as_str)
            .is_none()
        {
            return Err(format!(
                "entries[{}] : « request » avec « cmd » attendu",
                i + 1
            ));
        }
    }
    Ok(entries)
}

/// Rejoue un enregistrement sur une session neuve.
pub fn replay(
    document: &Value,
    material: &Material<'_>,
    session: &mut Session,
) -> Result<Report, String> {
    let entries = parse(document)?;
    let mut warnings = Vec::new();
    let app = document.get("app");
    if let Some(crc) = app.and_then(|a| a.get("crc32")).and_then(Value::as_str)
        && crc != crc_text(material.nwa)
    {
        warnings.push(format!(
            "l'application n'est pas celle de l'enregistrement (CRC {} au lieu de {crc})",
            crc_text(material.nwa)
        ));
    }
    if let Some(crc) = app
        .and_then(|a| a.get("data_crc32"))
        .and_then(Value::as_str)
    {
        match material.data {
            Some(d) if crc_text(d) == crc => {}
            Some(_) => warnings
                .push("les données externes ne sont pas celles de l'enregistrement".to_owned()),
            None => {
                warnings.push("l'enregistrement avait des données externes : les donner".to_owned())
            }
        }
    }
    if app.and_then(|a| a.get("flash")) == Some(&Value::Bool(true)) && material.flash.is_none() {
        warnings.push(
            "l'enregistrement avait des données auxiliaires en flash (.nfd) : les donner"
                .to_owned(),
        );
    }
    let mut load = Value::object()
        .with("cmd", "load")
        .with("nwa", json::base64_encode(material.nwa));
    if let Some(device) = document.get("device").and_then(Value::as_object) {
        for (key, value) in device {
            load.set(key, value.clone());
        }
    }
    if let Some(data) = material.data {
        load.set("data", json::base64_encode(data));
    }
    if let Some(flash) = material.flash {
        load.set("nfd", flash.clone());
    }
    let reply = session.handle(&load);
    if reply.get("ok") != Some(&Value::Bool(true)) {
        return Err(format!(
            "chargement impossible : {}",
            reply
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("erreur inconnue")
        ));
    }
    let (mut checks, mut mismatches) = (0, Vec::new());
    for (i, entry) in entries.iter().enumerate() {
        let request = entry.get("request").expect("vérifié par parse");
        let reply = session.handle(request);
        if reply.get("ok") != Some(&Value::Bool(true)) {
            mismatches.push(Mismatch {
                entry: i + 1,
                what: format!(
                    "{} : la requête échoue",
                    request.get("cmd").and_then(Value::as_str).unwrap_or("?")
                ),
                recorded: "succès".into(),
                replayed: reply
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("erreur")
                    .into(),
            });
            continue;
        }
        let Some(expect) = entry.get("expect").and_then(Value::as_object) else {
            continue;
        };
        for (key, wanted) in expect {
            checks += 1;
            let got = match key.as_str() {
                "stop" => reply.get("stop").and_then(|s| s.get("kind")).cloned(),
                "text_crc32" => reply
                    .get("text")
                    .and_then(Value::as_str)
                    .map(|t| Value::from(crc_text(t.as_bytes()))),
                other => reply.get(other).cloned(),
            };
            if got.as_ref() != Some(wanted) {
                mismatches.push(Mismatch {
                    entry: i + 1,
                    what: format!(
                        "{} : {key}",
                        request.get("cmd").and_then(Value::as_str).unwrap_or("?")
                    ),
                    recorded: wanted.to_string(),
                    replayed: got.map_or("(absent)".into(), |g| g.to_string()),
                });
            }
        }
    }
    Ok(Report {
        entries: entries.len(),
        checks,
        mismatches,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_document_without_entries_is_refused() {
        assert!(parse(&Value::object().with("nrp", 1u64)).is_err());
        assert!(
            parse(&Value::object().with("nfs", 1u64))
                .unwrap_err()
                .contains(".nrp")
        );
        let bad = Value::object()
            .with("nrp", 1u64)
            .with("entries", vec![Value::object()]);
        assert!(parse(&bad).unwrap_err().contains("entries[1]"));
    }

    #[test]
    fn only_state_changing_requests_are_recorded() {
        let mut r = Recorder::new();
        let ok = Value::object().with("ok", true);
        r.note(&Value::object().with("cmd", "cpu"), &ok);
        r.note(&Value::object().with("cmd", "key").with("key", "ok"), &ok);
        r.note(&Value::object().with("cmd", "memory_read"), &ok);
        r.note(
            &Value::object().with("cmd", "key").with("key", "back"),
            &Value::object().with("ok", false),
        );
        let doc = r.finish(Some("a.nwa"));
        assert_eq!(doc["entries"].as_array().unwrap().len(), 1);
        assert_eq!(doc["app"]["name"], "a.nwa");
    }
}
