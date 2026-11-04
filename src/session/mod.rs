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
//! `load` accepte aussi `"nfd"` (un document `.nfd` : des octets à placer en flash, voir `flash_load`).
//! `new` et `load` acceptent aussi `"clock"` (`"virtual"`, par défaut, ou `"host"`).
//! `new` et `load` acceptent `"model"` (`n0110`, `n0115`, `n0120`) et `"firmware"` (`"24"`, par
//! défaut, ou `"26.4.0"`).
//!
//! Commandes : `new`, `load`, `launch`, `run`, `key`, `advance`, `time`, `screen`, `stdout`, `cpu`,
//! `inspect`, `regions`, `usage`, `memory_read`, `memory_write`, `changes`, `watch`, `unwatch`,
//! `history`, `step_back`, `reverse_continue`, `history_goto`, `profile`, `backtrace`, `symbol`, `symbols`, `step`, `break`, `unbreak`, `breaks`, `cpu_set`, `disasm`, `watch_list`, `watch_hits`, `svc_stats`, `svc_log`, `fs_list`, `fs_stats`, `fs_read`, `fs_write`, `fs_remove`,
//! `fs_rename`, `flash_load`, `flash_export`, `settings_get`, `settings_set`, `help`.

mod aide;
mod appareil;
mod debug;
mod dispatch;
mod execution;
mod fichiers;
mod history;
mod observation;
mod reglages;
mod source;
mod views;

use crate::emulator::Emulator;
use crate::json::{self, Value, ValueExt};
use crate::kernel::Key;
use crate::kernel::{ClockMode, CpuSpeed, Kernel, KernelConfig};
use crate::profile::{FirmwareProfile, Model};

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
    /// Disposition du firmware de l'appareil (`"24"` ou `"26.4.0"`), pour enregistrer son état.
    firmware: String,
    /// Points de reprise pour remonter le temps, quand l'historique est lancé (voir `history`).
    history: Option<history::History>,
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

/// Numéro d'un registre de cœur d'après son nom (`r0` à `r12`, `sp`, `lr`, `pc`).
fn register_number(name: &str) -> Option<u8> {
    match name {
        "sp" => Some(13),
        "lr" => Some(14),
        "pc" => Some(15),
        other => other
            .strip_prefix('r')
            .and_then(|n| n.parse::<u8>().ok())
            .filter(|&n| n <= 12),
    }
}

fn register_name(number: u8) -> String {
    match number {
        13 => "sp".to_owned(),
        14 => "lr".to_owned(),
        15 => "pc".to_owned(),
        n => format!("r{n}"),
    }
}

fn model_of(name: &str) -> Result<Model, String> {
    Model::ALL
        .into_iter()
        .find(|m| m.name().eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("modèle inconnu : {name} (n0110, n0115 ou n0120)"))
}

/// Disposition du firmware demandée par `firmware` : `\"24\"` (appareils relevés, par défaut) ou
/// `\"26.4.0\"` (firmware officiel récent : les applications qui devinent le modèle d'après la
/// flash, comme celles écrites avec eadkp, s'y trompent sur N0110 et N0115).
fn profile_of(req: &Value, model: Model) -> Result<FirmwareProfile, String> {
    let layout = req.get("firmware").and_then(Value::as_str).unwrap_or("24");
    FirmwareProfile::for_layout(layout, model)
        .ok_or_else(|| format!("firmware inconnu : {layout} (24 ou 26.4.0)"))
}

/// Fréquence personnalisée maximale acceptée, en MHz (une valeur absurde est une erreur de frappe).
const MAX_CUSTOM_MHZ: u32 = 100_000;

/// Réglages du noyau demandés par `clock` : `"virtual"` (défaut, déterministe) ou `"host"` (le temps
/// n'avance que par la commande `advance`, que l'hôte appelle avec le temps réellement écoulé), et
/// par `speed` / `mhz` (voir [`apply_speed`]).
fn config_of(req: &Value) -> Result<KernelConfig, String> {
    let clock = match req
        .get("clock")
        .and_then(Value::as_str)
        .unwrap_or("virtual")
    {
        "virtual" => ClockMode::Virtual,
        "host" => ClockMode::Host,
        other => return Err(format!("horloge inconnue : {other} (virtual ou host)")),
    };
    let mut config = KernelConfig {
        clock,
        ..KernelConfig::default()
    };
    apply_speed(req, &mut config)?;
    Ok(config)
}

/// Applique à `config` les champs `speed` (`"model"`, `"custom"` ou `"max"`) et `mhz` (la fréquence
/// personnalisée, ignorée hors du mode `"custom"`) de la requête ; un champ absent ne change rien.
fn apply_speed(req: &Value, config: &mut KernelConfig) -> Result<(), String> {
    if let Some(value) = req.get("speed") {
        config.speed = match value.as_str() {
            Some("model") => CpuSpeed::Model,
            Some("custom") => CpuSpeed::Custom,
            Some("max") => CpuSpeed::Max,
            _ => return Err("vitesse inconnue : model, custom ou max".to_owned()),
        };
    }
    if req.get("mhz").is_some() {
        let mhz = number(req, "mhz")?;
        if !(1..=MAX_CUSTOM_MHZ).contains(&mhz) {
            return Err(format!("« mhz » doit être entre 1 et {MAX_CUSTOM_MHZ}"));
        }
        config.custom_mhz = mhz;
    }
    Ok(())
}

/// Ajoute à une réponse la vitesse du processeur : `speed` (le mode), `mhz` (la fréquence
/// personnalisée), `cpu_mhz` (la fréquence du temps virtuel) et `limit_mhz` (celle que l'hôte doit
/// appliquer, `null` quand la vitesse n'est pas limitée).
fn with_speed(value: Value, kernel: &Kernel) -> Value {
    let name = match kernel.config.speed {
        CpuSpeed::Model => "model",
        CpuSpeed::Custom => "custom",
        CpuSpeed::Max => "max",
    };
    value
        .with("speed", name)
        .with("mhz", kernel.config.custom_mhz)
        .with("cpu_mhz", kernel.cycles_per_us())
        .with("limit_mhz", kernel.limit_mhz())
}

fn key_of(req: &Value) -> Result<Key, String> {
    let name = text(req, "key")?;
    Key::from_name(name).ok_or_else(|| format!("touche inconnue : {name}"))
}

/// Corps d'un fichier donné en `text` ou en `content` (octets).
fn body_of(req: &Value) -> Result<Vec<u8>, String> {
    if let Some(t) = req.get("text").and_then(Value::as_str) {
        return Ok(t.as_bytes().to_vec());
    }
    let b64 = text(req, "content").map_err(|_| "il faut « text » ou « content »".to_owned())?;
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

    /// Les pixels de l'écran (RGB565, 320×240, ligne par ligne), s'il y a un appareil. Sert aux
    /// hôtes qui dessinent à chaque image sans passer par du JSON.
    pub fn framebuffer(&self) -> Option<&[u16]> {
        self.emu.as_ref().map(|e| e.kernel.display.pixels())
    }

    /// Traite une requête et rend la réponse.
    pub fn handle(&mut self, request: &Value) -> Value {
        let result = self.dispatch(request);
        if result.is_ok() && self.history.is_some() {
            let cmd = request.get("cmd").and_then(Value::as_str).unwrap_or("");
            let tap = cmd == "key"
                && request
                    .get("action")
                    .and_then(Value::as_str)
                    .unwrap_or("tap")
                    == "tap";
            match history::effect_of(cmd) {
                history::Effect::Input if !tap => self.history_record(request),
                history::Effect::Reset | history::Effect::Input => self.history_reset(),
                history::Effect::None | history::Effect::Forward => {}
            }
        }
        let mut reply = match result {
            Ok(v) => merged(Value::object().with("ok", true), v),
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
                .with("error", format!("JSON invalide : {e}"))
                .to_string(),
        }
    }
}

/// Ajoute les clés de `other` (un objet) à `base`.
fn merged(mut base: Value, other: Value) -> Value {
    if let Value::Object(entries) = other {
        for (k, v) in entries {
            base.set(&k, v);
        }
    }
    base
}
