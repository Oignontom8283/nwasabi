//! Le protocole de session : une conversation JSON avec un appareil émulé.
//!
//! Une requête est un objet `{"cmd": "...", ...}` (avec un `"id"` facultatif, renvoyé tel quel) ;
//! la réponse est un objet `{"ok": true, ...}` ou `{"ok": false, "error": "..."}`. C'est le même
//! contrat pour l'outil en ligne de commande (`nwasabi-cli session`), le serveur et la page web,
//! qui n'ont donc qu'à transporter ces objets.
//!
//! La session ne touche ni au disque ni à l'horloge : les fichiers entrent et sortent en base64
//! (ou en hexadécimal pour la mémoire), le temps est donné par `advance`.
//!
//! Commandes : `new`, `load`, `launch`, `run`, `key`, `advance`, `screen`, `stdout`, `cpu`,
//! `inspect`, `regions`, `usage`, `memory_read`, `memory_write`, `changes`, `watch`, `unwatch`,
//! `watch_hits`, `svc_stats`, `fs_list`, `fs_stats`, `fs_read`, `fs_write`, `fs_remove`,
//! `fs_rename`, `settings_get`, `settings_set`, `help`.

mod views;

use crate::emulator::{Emulator, RunStop};
use crate::inspect::read_structures;
use crate::json::{self, Value};
use crate::kernel::Key;
use crate::memory::WatchKind;
use crate::png;
use crate::profile::{Model, Slot};
use crate::storage::PythonSource;

/// Budget d'instructions par défaut d'une commande `run`.
const DEFAULT_INSTRUCTIONS: u64 = 10_000_000;
/// Instructions exécutées par pas pendant `run` (le temps de réaction aux arrêts).
const CHUNK: u64 = 1_000_000;
/// Taille maximale lue ou écrite d'un coup en mémoire.
const MAX_TRANSFER: u32 = 1 << 20;

type Reply = Result<Value, String>;

/// Un appareil émulé et ce qu'il faut pour répondre aux commandes.
#[derive(Default)]
pub struct Session {
    emu: Option<Emulator>,
    frames: u64,
}

fn text<'a>(req: &'a Value, key: &str) -> Result<&'a str, String> {
    req.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("champ « {key} » (texte) manquant"))
}

fn number(req: &Value, key: &str) -> Result<u32, String> {
    req.get(key)
        .and_then(Value::as_u32)
        .ok_or_else(|| format!("champ « {key} » (entier) manquant ou invalide"))
}

fn model_of(name: &str) -> Result<Model, String> {
    Model::ALL
        .into_iter()
        .find(|m| m.name().eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("modèle inconnu : {name} (n0110, n0115 ou n0120)"))
}

fn key_of(req: &Value) -> Result<Key, String> {
    let name = text(req, "key")?;
    Key::from_name(name).ok_or_else(|| format!("touche inconnue : {name}"))
}

/// Corps d'un fichier donné en `text` ou en `base64`.
fn body_of(req: &Value) -> Result<Vec<u8>, String> {
    if let Some(t) = req.get("text").and_then(Value::as_str) {
        return Ok(t.as_bytes().to_vec());
    }
    let b64 = text(req, "base64").map_err(|_| "il faut « text » ou « base64 »".to_owned())?;
    json::base64_decode(b64).ok_or_else(|| "base64 invalide".to_owned())
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    fn emu(&mut self) -> Result<&mut Emulator, String> {
        self.emu
            .as_mut()
            .ok_or_else(|| "aucun appareil : utiliser « new » ou « load »".to_owned())
    }

    /// Traite une requête et rend la réponse.
    pub fn handle(&mut self, request: &Value) -> Value {
        let mut reply = match self.dispatch(request) {
            Ok(v) => Value::object().with("ok", true).merged(v),
            Err(e) => Value::object().with("ok", false).with("error", e),
        };
        if let Some(id) = request.get("id") {
            reply.set("id", id.clone());
        }
        reply
    }

    /// Variante pour du texte : une ligne JSON en entrée, une ligne JSON en sortie.
    pub fn handle_line(&mut self, line: &str) -> String {
        match json::parse(line) {
            Ok(request) => self.handle(&request).to_string(),
            Err(e) => Value::object()
                .with("ok", false)
                .with("error", e.to_string())
                .to_string(),
        }
    }

    fn dispatch(&mut self, req: &Value) -> Reply {
        let cmd = text(req, "cmd")?;
        match cmd {
            "help" => Ok(Value::object().with(
                "commands",
                vec![
                    "new",
                    "load",
                    "launch",
                    "run",
                    "key",
                    "advance",
                    "screen",
                    "stdout",
                    "cpu",
                    "inspect",
                    "regions",
                    "usage",
