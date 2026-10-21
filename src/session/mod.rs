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
                    "memory_read",
                    "memory_write",
                    "changes",
                    "watch",
                    "unwatch",
                    "watch_hits",
                    "svc_stats",
                    "fs_list",
                    "fs_stats",
                    "fs_read",
                    "fs_write",
                    "fs_remove",
                    "fs_rename",
                    "settings_get",
                    "settings_set",
                ],
            )),
            "new" => self.new_device(req),
            "load" => self.load(req),
            "launch" => {
                let emu = self.emu()?;
                emu.launch().map_err(|e| e.to_string())?;
                Ok(Value::object())
            }
            "run" => self.run(req),
            "key" => self.key(req),
            "advance" => {
                let ms = number(req, "ms")?;
                self.emu()?.kernel.advance_time(u64::from(ms) * 1000);
                Ok(Value::object().with("time_ms", self.time_ms()?))
            }
            "screen" => self.screen(req),
            "stdout" => {
                let emu = self.emu()?;
                let out = String::from_utf8_lossy(&emu.kernel.stdout).into_owned();
                if req.get("clear").and_then(Value::as_bool) == Some(true) {
                    emu.kernel.stdout.clear();
                }
                Ok(Value::object().with("text", out))
            }
            "cpu" => {
                let emu = self.emu()?;
                let r = &emu.cpu.regs;
                Ok(Value::object()
                    .with("r", r.r.iter().map(|&v| Value::from(v)).collect::<Vec<_>>())
                    .with("sp", r.sp)
                    .with("lr", r.lr)
                    .with("pc", r.pc)
                    .with("xpsr", r.xpsr)
                    .with("fpscr", r.fpscr)
                    .with("instructions", emu.cpu.cycles))
            }
            "inspect" => {
                let emu = self.emu()?;
                let exam = emu.device.exam_mode;
                let structures = read_structures(&mut emu.device.memory, exam);
                let mut out = Value::object().with("structures", views::structures(&structures));
                if let Some(app) = emu.app() {
                    out.set("app", views::app(app));
                }
                Ok(out)
            }
            "regions" => {
                let emu = self.emu()?;
                Ok(Value::object().with("regions", views::regions(&emu.device.memory.regions())))
            }
            "usage" => {
                let emu = self.emu()?;
                let usage = emu.usage().ok_or("aucune application installée")?;
                Ok(views::usage(&usage))
            }
            "memory_read" => {
                let (address, length) = (number(req, "address")?, number(req, "length")?);
                if length > MAX_TRANSFER {
                    return Err(format!("lecture limitée à {MAX_TRANSFER} octets"));
                }
                let bytes = self
                    .emu()?
                    .device
                    .memory
                    .read_bytes(address, length)
                    .map_err(|e| e.to_string())?;
                Ok(Value::object().with("hex", json::hex_encode(&bytes)))
            }
            "memory_write" => {
                let address = number(req, "address")?;
                let bytes = json::hex_decode(text(req, "hex")?).ok_or("hexadécimal invalide")?;
                if bytes.len() as u32 > MAX_TRANSFER {
                    return Err(format!("écriture limitée à {MAX_TRANSFER} octets"));
                }
                self.emu()?
                    .device
                    .memory
                    .write_bytes(address, &bytes)
                    .map_err(|e| e.to_string())?;
                Ok(Value::object().with("written", bytes.len()))
            }
            "changes" => {
                let memory = &self.emu()?.device.memory;
                let since = req
                    .get("cursor")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    .max(0) as u64;
                let ranges = memory
                    .changes_since(since)
                    .iter()
                    .map(|r| Value::object().with("start", r.start).with("len", r.len))
                    .collect::<Vec<_>>();
                Ok(Value::object()
                    .with("ranges", ranges)
                    .with("cursor", memory.change_cursor()))
            }
            "watch" => {
                let (start, len) = (number(req, "address")?, number(req, "length")?);
                let kind = match req.get("kind").and_then(Value::as_str).unwrap_or("write") {
                    "read" => WatchKind::Read,
                    "write" => WatchKind::Write,
                    "readwrite" => WatchKind::ReadWrite,
                    other => {
                        return Err(format!("sorte inconnue : {other} (read, write, readwrite)"));
                    }
                };
                let stop = req.get("stop").and_then(Value::as_bool).unwrap_or(false);
                let id = self.emu()?.device.memory.watch(start, len, kind, stop);
                Ok(Value::object().with("id", id.0))
            }
            "unwatch" => {
                let id = crate::memory::WatchId(number(req, "id")?);
                let removed = self.emu()?.device.memory.unwatch(id);
                Ok(Value::object().with("removed", removed))
            }
            "watch_hits" => {
                let hits = self.emu()?.device.memory.take_watch_hits();
                Ok(Value::object().with(
                    "hits",
                    hits.iter()
                        .map(|h| {
                            Value::object()
                                .with("id", h.id.0)
                                .with("address", h.address)
                                .with("width", u32::from(h.width))
                                .with("write", h.write)
                                .with("value", h.value)
                        })
                        .collect::<Vec<_>>(),
                ))
            }
            "svc_stats" => {
                let emu = self.emu()?;
                let counts = emu
                    .kernel
                    .svc_counts
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c > 0)
                    .map(|(n, c)| Value::object().with("svc", n).with("calls", *c))
                    .collect::<Vec<_>>();
                Ok(Value::object()
                    .with("counts", counts)
                    .with("frames", emu.kernel.frames))
            }
            c if c.starts_with("fs_") => self.fs(c, req),
            c if c.starts_with("settings_") => self.settings(c, req),
            other => Err(format!("commande inconnue : {other} (voir « help »)")),
        }
    }

