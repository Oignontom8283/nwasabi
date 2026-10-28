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

mod debug;
mod history;
mod views;

use crate::checksum;
use crate::elf::SymbolKind;
use crate::emulator::{Emulator, RunStop};
use crate::formats::{nds, nfd, nfs};
use crate::inspect::read_structures;
use crate::json::{self, Value, ValueExt};
use crate::kernel::Key;
use crate::kernel::{ClockMode, CpuSpeed, Kernel, KernelConfig};
use crate::memory::WatchKind;
use crate::profile::{FirmwareProfile, Model, Slot};
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
                    "speed",
                    "time",
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
                    "history",
                    "step_back",
                    "reverse_continue",
                    "history_goto",
                    "profile",
                    "backtrace",
                    "symbol",
                    "symbols",
                    "step",
                    "break",
                    "unbreak",
                    "breaks",
                    "cpu_set",
                    "disasm",
                    "watch_list",
                    "watch_hits",
                    "svc_stats",
                    "svc_log",
                    "fs_list",
                    "fs_stats",
                    "fs_read",
                    "fs_write",
                    "fs_remove",
                    "fs_rename",
                    "fs_export",
                    "fs_import",
                    "flash_load",
                    "flash_export",
                    "dump",
                    "restore",
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
            "profile" => {
                let action = req.get("action").and_then(Value::as_str).unwrap_or("get");
                let top = req
                    .get("top")
                    .and_then(Value::as_u32)
                    .unwrap_or(30)
                    .min(200) as usize;
                let emu = self.emu()?;
                match action {
                    "start" => {
                        emu.start_profile();
                        Ok(Value::object().with("running", true))
                    }
                    "stop" => {
                        let samples = emu.stop_profile().unwrap_or_default();
                        Ok(debug::profile_report(emu, &samples, top).with("running", false))
                    }
                    "get" => {
                        let running = emu.profile_samples().is_some();
                        let samples = emu.profile_samples().unwrap_or_default();
                        Ok(debug::profile_report(emu, &samples, top).with("running", running))
                    }
                    other => Err(format!("action inconnue : {other} (start, stop, get)")),
                }
            }
            "backtrace" => {
                let max = req
                    .get("max")
                    .and_then(Value::as_u32)
                    .unwrap_or(32)
                    .min(256) as usize;
                Ok(debug::backtrace(self.emu()?, max))
            }
            "symbols" => {
                let query = req
                    .get("query")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_lowercase();
                let emu = self.emu()?;
                let found = emu
                    .app()
                    .map(|a| a.symbols.as_slice())
                    .unwrap_or_default()
                    .iter()
                    .filter(|s| {
                        s.kind == SymbolKind::Func && s.name.to_lowercase().contains(&query)
                    })
                    .take(500)
                    .map(|s| {
                        Value::object()
                            .with("name", s.name.as_str())
                            .with("address", s.address & !1)
                            .with("size", s.size)
                    })
                    .collect::<Vec<_>>();
                Ok(Value::object().with("symbols", found))
            }
            "symbol" => {
                let address = number(req, "address")?;
                let emu = self.emu()?;
                match emu.symbol_at(address) {
                    Some((name, start, offset)) => Ok(Value::object()
                        .with("name", name)
                        .with("start", start)
                        .with("offset", offset)),
                    None => Ok(Value::object()),
                }
            }
            "history" => self.history_command(req),
            "step_back" => self.step_back(),
            "reverse_continue" => self.reverse_continue(),
            "history_goto" => self.history_goto(req),
            "step" => {
                let emu = self.emu()?;
                if emu.app().is_none() {
                    return Err("aucune application installée".to_owned());
                }
                let start = emu.cpu.cycles;
                self.history_fork();
                let emu = self.emu()?;
                let mut stop = emu.step();
                // Une instruction qui endort l'application (`usleep`…) se refait tant que le temps n'a pas passé :
                // pour qu'un pas la fasse vraiment avancer, on laisse passer le temps demandé et on la refait.
                let mut advanced = Vec::new();
                for _ in 0..8 {
                    let RunStop::Sleep { ms } = stop else { break };
                    emu.kernel.advance_time(u64::from(ms) * 1000);
                    advanced.push(ms);
                    stop = emu.step();
                }
                let reply = Value::object()
                    .with("stop", debug::stop_json(emu, &stop))
                    .with("instructions", emu.cpu.cycles - start)
                    .with("time_ms", emu.time_us() / 1000);
                // Le temps passé ainsi est une décision de l'hôte : l'historique le garde pour le rejeu.
                for ms in advanced {
                    self.history_record(&Value::object().with("cmd", "advance").with("ms", ms));
                }
                self.history_tick();
                Ok(reply)
            }
            "break" => {
                let address = number(req, "address")?;
                // Condition facultative : { "register": "r0", "op": "==", "value": 5 }.
                let condition = match req.get("condition") {
                    Some(c) if !matches!(c, Value::Null) => {
                        let name = text(c, "register")?.to_ascii_lowercase();
                        let register = register_number(&name)
                            .ok_or_else(|| format!("registre inconnu : {name}"))?;
                        let op = text(c, "op")?;
                        let compare = crate::emulator::Compare::parse(op).ok_or_else(|| {
                            format!("comparaison inconnue : {op} (==, !=, <, <=, >, >=, &)")
                        })?;
                        Some(crate::emulator::Condition {
                            register,
                            compare,
                            value: number(c, "value")?,
                        })
                    }
                    _ => None,
                };
                self.emu()?.add_conditional_breakpoint(address, condition);
                Ok(Value::object())
            }
            "unbreak" => {
                let address = number(req, "address")?;
                let removed = self.emu()?.remove_breakpoint(address);
                Ok(Value::object().with("removed", removed))
            }
            "breaks" => Ok(Value::object()
                .with(
                    "breakpoints",
                    self.emu()?
                        .breakpoints()
                        .iter()
                        .map(|b| Value::from(b.address))
                        .collect::<Vec<_>>(),
                )
                .with(
                    "details",
                    self.emu()?
                        .breakpoints()
                        .iter()
                        .map(|b| {
                            let mut v = Value::object().with("address", b.address);
                            if let Some(c) = b.condition {
                                v.set(
                                    "condition",
                                    Value::object()
                                        .with("register", register_name(c.register))
                                        .with("op", c.compare.symbol())
                                        .with("value", c.value),
                                );
                            }
                            v
                        })
                        .collect::<Vec<_>>(),
                )),
            "cpu_set" => {
                let name = text(req, "register")?.to_ascii_lowercase();
                let value = number(req, "value")?;
                let r = &mut self.emu()?.cpu.regs;
                match name.as_str() {
                    "sp" => r.sp = value & !3,
                    "lr" => r.lr = value,
                    "pc" => r.pc = value & !1,
                    "xpsr" => r.xpsr = value,
                    other => {
                        let index = other
                            .strip_prefix('r')
                            .and_then(|n| n.parse::<usize>().ok())
                            .filter(|&n| n < r.r.len())
                            .ok_or_else(|| {
                                format!("registre inconnu : {other} (r0 à r12, sp, lr, pc, xpsr)")
                            })?;
                        r.r[index] = value;
                    }
                }
                Ok(Value::object())
            }
            "advance" => {
                // `us` (microsecondes) ou `ms` : le temps à faire passer pour l'application.
                let us = match req.get("us") {
                    Some(_) => u64::from(number(req, "us")?),
                    None => u64::from(number(req, "ms")?) * 1000,
                };
                self.emu()?.kernel.advance_time(us);
                Ok(Value::object().with("time_ms", self.time_ms()?))
            }
            "speed" => {
                // Change la vitesse du processeur (et/ou la fréquence personnalisée) ; sans champ, la rend.
                let emu = self.emu()?;
                apply_speed(req, &mut emu.kernel.config)?;
                Ok(with_speed(Value::object(), &emu.kernel))
            }
            "time" => {
                // Temps de simulation depuis le début : celui de l'application, lisible à tout moment.
                let us = self.emu()?.time_us();
                Ok(Value::object()
                    .with("time_us", us)
                    .with("time_ms", us / 1000))
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
                    .with("s", r.s.iter().map(|&v| Value::from(v)).collect::<Vec<_>>())
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
            "disasm" => {
                let address = number(req, "address")? & !1;
                let count = req
                    .get("count")
                    .and_then(Value::as_u32)
                    .unwrap_or(16)
                    .clamp(1, 256) as usize;
                let before = req
                    .get("before")
                    .and_then(Value::as_u32)
                    .unwrap_or(0)
                    .min(32) as usize;
                let memory = &mut self.emu()?.device.memory;
                let mut read = |a: u32| {
                    let bytes = memory.read_bytes(a, 2).ok()?;
                    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
                };
                // Les instructions Thumb ont 2 ou 4 octets : pour montrer ce qui précède, on part de plus loin et l'on
                // garde le plus long départ dont la lecture tombe exactement sur `address`.
                let mut lines = Vec::new();
                let mut index = 0;
                for back in (0..=2 * before).rev() {
                    let start = address.wrapping_sub(2 * back as u32);
                    let attempt = crate::cpu::disassemble(&mut read, start, back + count);
                    if let Some(at) = attempt.iter().position(|l| l.address == address) {
                        index = at;
                        lines = attempt;
                        break;
                    }
                }
                let from = index.saturating_sub(before);
                let emu = &*self.emu()?;
                let rows = lines
                    .iter()
                    .skip(from)
                    .take(before.min(index) + count)
                    .map(|l| {
                        let hex = if l.size == 4 {
                            format!("{:04x} {:04x}", l.halfwords[0], l.halfwords[1])
                        } else {
                            format!("{:04x}", l.halfwords[0])
                        };
                        let mut row = Value::object()
                            .with("address", l.address)
                            .with("size", u32::from(l.size))
                            .with("hex", hex.as_str())
                            .with("mnemonic", l.mnemonic.as_str())
                            .with("operands", l.operands.as_str());
                        if let Some(target) = l.target {
                            row.set("target", Value::from(target));
                            if let Some(text) = debug::symbol_text(emu, target) {
                                row.set("target_symbol", Value::from(text));
                            }
                        }
                        if let Some((name, start, _)) = emu.symbol_at(l.address)
                            && start == l.address
                        {
                            row.set("label", Value::from(name));
                        }
                        row
                    })
                    .collect::<Vec<_>>();
                Ok(Value::object().with("lines", rows))
            }
            "watch_list" => {
                let list = self.emu()?.device.memory.watch_list();
                Ok(Value::object().with(
                    "watches",
                    list.iter()
                        .map(|(id, start, len, kind, stop)| {
                            Value::object()
                                .with("id", id.0)
                                .with("address", *start)
                                .with("length", *len)
                                .with(
                                    "kind",
                                    match kind {
                                        WatchKind::Read => "read",
                                        WatchKind::Write => "write",
                                        WatchKind::ReadWrite => "readwrite",
                                    },
                                )
                                .with("stop", *stop)
                        })
                        .collect::<Vec<_>>(),
                ))
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
                                .with("pc", h.pc)
                        })
                        .collect::<Vec<_>>(),
                ))
            }
            "svc_log" => {
                let action = req.get("action").and_then(Value::as_str).unwrap_or("get");
                let since = req
                    .get("since")
                    .and_then(Value::as_u32)
                    .map_or(0, u64::from);
                let kernel = &mut self.emu()?.kernel;
                match action {
                    "start" => kernel.svc_log = Some(crate::kernel::SvcLog::default()),
                    "stop" => kernel.svc_log = None,
                    "clear" => {
                        if let Some(log) = kernel.svc_log.as_mut() {
                            log.calls.clear();
                        }
                    }
                    "get" => {}
                    other => {
                        return Err(format!(
                            "action inconnue : {other} (start, stop, clear, get)"
                        ));
                    }
                }
                let (running, total, calls) = match &kernel.svc_log {
                    Some(log) => (
                        true,
                        log.total,
                        log.calls
                            .iter()
                            .filter(|c| c.seq >= since)
                            .map(|c| {
                                Value::object()
                                    .with("seq", c.seq)
                                    .with("svc", u32::from(c.number))
                                    .with("pc", c.pc)
                                    .with(
                                        "args",
                                        c.args.iter().map(|&a| Value::from(a)).collect::<Vec<_>>(),
                                    )
                                    .with("ret", c.ret)
                                    .with("frame", c.frame)
                            })
                            .collect::<Vec<_>>(),
                    ),
                    None => (false, 0, Vec::new()),
                };
                Ok(Value::object()
                    .with("running", running)
                    .with("total", total)
                    .with("calls", calls))
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
            "fs_export" => {
                let doc = nfs::export(&mut self.emu()?.fs())?;
                Ok(Value::object().with("nfs", doc))
            }
            "fs_import" => {
                let doc = req.get("nfs").ok_or("champ « nfs » (document) manquant")?;
                let replace = req.get("replace").and_then(Value::as_bool).unwrap_or(true);
                let count = nfs::import(&mut self.emu()?.fs(), doc, replace)?;
                Ok(Value::object().with("files", count))
            }
            "flash_load" => {
                let doc = req.get("nfd").ok_or("champ « nfd » (document) manquant")?;
                let segments = nfd::parse(doc)?;
                let count = segments.len();
                self.emu()?
                    .install_flash(segments)
                    .map_err(|e| e.to_string())?;
                Ok(Value::object().with("segments", count))
            }
            "flash_export" => {
                let doc = nfd::export(self.emu()?.flash_segments());
                Ok(Value::object().with("nfd", doc))
            }
            "dump" => self.dump(req),
            "restore" => self.restore(req),
            c if c.starts_with("fs_") => self.fs(c, req),
            c if c.starts_with("settings_") => self.settings(c, req),
            other => Err(format!("commande inconnue : {other} (voir « help »)")),
        }
    }

    /// Enregistre l'état de l'appareil (format `.nds`). `parts` (liste parmi `app`, `memory`, `fs`, `state`)
    /// choisit ce qui est gardé ; par défaut, tout.
    fn dump(&mut self, req: &Value) -> Reply {
        let parts = match req.get("parts") {
            None => nds::Parts::default(),
            Some(list) => {
                let list = list
                    .as_array()
                    .ok_or("« parts » : une liste est attendue")?;
                let mut parts = nds::Parts {
                    app: false,
                    memory: false,
                    fs: false,
                    state: false,
                };
                for part in list {
                    match part.as_str() {
                        Some("app") => parts.app = true,
                        Some("memory") => parts.memory = true,
                        Some("fs") => parts.fs = true,
                        Some("state") => parts.state = true,
                        _ => return Err("« parts » : app, memory, fs ou state".to_owned()),
                    }
                }
                parts
            }
        };
        let firmware = self.firmware().to_owned();
        let doc = nds::dump(self.emu()?, &firmware, parts)?;
        Ok(Value::object().with("nds", doc))
    }

    /// Remplace l'appareil par l'état d'un document `.nds`. Sans `.nwa` dans le document, celui de l'appareil
    /// actuel est gardé.
    fn restore(&mut self, req: &Value) -> Reply {
        let doc = req.get("nds").ok_or("champ « nds » (document) manquant")?;
        let current = self.emu.as_mut().map(nds::current);
        let restored = nds::restore(doc, current.as_ref())?;
        let reply = with_speed(
            Value::object()
                .with("model", restored.emulator.device.profile.model.name())
                .with("firmware", restored.firmware.as_str())
                .with("frames", restored.emulator.kernel.frames)
                .with(
                    "parts",
                    restored
                        .parts
                        .iter()
                        .map(|p| Value::from(*p))
                        .collect::<Vec<_>>(),
                ),
            &restored.emulator.kernel,
        );
        let reply = match restored.emulator.app() {
            Some(app) => reply.with("app", views::app(app)),
            None => reply,
        };
        self.frames = restored.emulator.kernel.frames;
        self.firmware = restored.firmware;
        self.emu = Some(restored.emulator);
        Ok(reply)
    }

    fn firmware(&self) -> &str {
        if self.firmware.is_empty() {
            "24"
        } else {
            &self.firmware
        }
    }

    fn time_ms(&mut self) -> Result<u64, String> {
        let emu = self.emu()?;
        Ok(emu.time_us() / 1000)
    }

    fn new_device(&mut self, req: &Value) -> Reply {
        let model = model_of(req.get("model").and_then(Value::as_str).unwrap_or("n0110"))?;
        let profile = profile_of(req, model)?;
        let emu = Emulator::with_profile(profile, config_of(req)?).map_err(|e| e.to_string())?;
        let reply = with_speed(Value::object().with("model", model.name()), &emu.kernel);
        self.firmware = req
            .get("firmware")
            .and_then(Value::as_str)
            .unwrap_or("24")
            .to_owned();
        self.emu = Some(emu);
        self.frames = 0;
        Ok(reply)
    }

    fn load(&mut self, req: &Value) -> Reply {
        let nwa = json::base64_decode(text(req, "nwa")?).ok_or("base64 invalide")?;
        let data = match req.get("data").and_then(Value::as_str) {
            Some(d) => Some(json::base64_decode(d).ok_or("base64 invalide (data)")?),
            None => None,
        };
        let model = model_of(req.get("model").and_then(Value::as_str).unwrap_or("n0110"))?;
        let mut profile = profile_of(req, model)?;
        // Des octets à placer en flash (`.nfd`), vérifiés avant de construire l'appareil.
        let flash = match req.get("nfd") {
            Some(doc) => {
                if let Some(slot) = nfd::active_slot(doc)? {
                    profile.active_slot = slot;
                }
                nfd::parse(doc)?
            }
            None => Vec::new(),
        };
        let mut emu =
            Emulator::with_profile(profile, config_of(req)?).map_err(|e| e.to_string())?;
        // Placés avant l'application, ils sont vérifiés contre elle quand elle s'installe.
        emu.install_flash(flash).map_err(|e| e.to_string())?;
        emu.install_nwa(&nwa, data.as_deref())
            .map_err(|e| e.to_string())?;
        let info = views::app(emu.app().expect("application installée"));
        let reply = with_speed(
            Value::object()
                .with("model", model.name())
                .with("app", info),
            &emu.kernel,
        );
        self.firmware = req
            .get("firmware")
            .and_then(Value::as_str)
            .unwrap_or("24")
            .to_owned();
        self.emu = Some(emu);
        self.frames = 0;
        Ok(reply)
    }

    /// Exécute jusqu'à un arrêt qui demande l'hôte, `frames` images ou `instructions` instructions.
    fn run(&mut self, req: &Value) -> Reply {
        let budget = req
            .get("instructions")
            .and_then(Value::as_i64)
            .map_or(DEFAULT_INSTRUCTIONS, |n| n.max(0) as u64);
        let frames = req
            .get("frames")
            .and_then(Value::as_i64)
            .map(|n| n.max(0) as u64);
        let emu = self
            .emu
            .as_mut()
            .ok_or("aucun appareil : utiliser « new » ou « load »")?;
        if emu.app().is_none() {
            return Err("aucune application installée".to_owned());
        }
        let start_cycles = emu.cpu.cycles;
        let host_clock = emu.kernel.config.clock == ClockMode::Host;
        self.history_fork();
        let mut seen = 0u64;
        let stop = loop {
            // Avec un historique, les tranches sont assez courtes pour ne pas manquer l'heure d'un point de reprise.
            let chunk = CHUNK.min(self.history_slice().unwrap_or(CHUNK));
            let emu = self.emu.as_mut().ok_or("aucun appareil")?;
            let used = emu.cpu.cycles - start_cycles;
            if used >= budget {
                break RunStop::Budget;
            }
            let stop = emu.run(chunk.min(budget - used));
            self.history_tick();
            match stop {
                RunStop::Frame => {
                    seen += 1;
                    if frames.is_some_and(|f| seen >= f) {
                        break stop;
                    }
                }
                // Avec l'horloge de l'hôte une attente ne finit que si l'hôte fait passer du temps :
                // inutile d'insister, on lui rend la main.
                RunStop::Sleep { .. } if host_clock => break stop,
                RunStop::Sleep { .. } | RunStop::Budget => {}
                other => break other,
            }
        };
        self.frames += seen;
        let emu = self.emu()?;
        Ok(Value::object()
            .with("stop", debug::stop_json(emu, &stop))
            .with("frames", seen)
            .with("instructions", emu.cpu.cycles - start_cycles)
            .with("time_ms", emu.time_us() / 1000))
    }

    fn key(&mut self, req: &Value) -> Reply {
        let key = key_of(req)?;
        let action = req.get("action").and_then(Value::as_str).unwrap_or("tap");
        let emu = self.emu()?;
        match action {
            "press" => emu.kernel.keyboard.press(key),
            "release" => emu.kernel.keyboard.release(key),
            "tap" => {
                emu.kernel.keyboard.press(key);
                let hold = req
                    .get("instructions")
                    .and_then(Value::as_i64)
                    .unwrap_or(2_000_000);
                emu.run(hold.max(0) as u64);
                emu.kernel.keyboard.release(key);
            }
            other => return Err(format!("action inconnue : {other} (press, release, tap)")),
        }
        Ok(Value::object().with("state", emu.kernel.keyboard.state()))
    }

    fn screen(&mut self, req: &Value) -> Reply {
        let emu = self.emu()?;
        let display = &emu.kernel.display;
        let bytes: Vec<u8> = display
            .pixels()
            .iter()
            .flat_map(|p| p.to_le_bytes())
            .collect();
        let mut out = Value::object()
            .with("width", crate::kernel::display::WIDTH)
            .with("height", crate::kernel::display::HEIGHT)
            .with("crc32", format!("{:08x}", checksum::crc32(&bytes)))
            .with(
                "lit_pixels",
                display.pixels().iter().filter(|&&p| p != 0).count(),
            );
        if req.get("png").is_some() {
            return Err(
                "l'encodage PNG se fait côté hôte : demander « rgba » (pixels RGBA bruts)"
                    .to_owned(),
            );
        }
        if req.get("rgba").and_then(Value::as_bool) == Some(true) {
            out.set("rgba", json::base64_encode(&display.to_rgba()));
        }
        if req.get("rgb565").and_then(Value::as_bool) == Some(true) {
            out.set("rgb565", json::base64_encode(&bytes));
        }
        Ok(out)
    }

    fn fs(&mut self, cmd: &str, req: &Value) -> Reply {
        let emu = self.emu()?;
        let mut fs = emu.fs();
        let e = |e: crate::storage::FsError| e.to_string();
        match cmd {
            "fs_list" => Ok(views::listing(&fs.list().map_err(e)?)
                .with("generation", fs.generation().map_err(e)?)),
            "fs_stats" => Ok(views::stats(&fs.stats().map_err(e)?)),
            "fs_read" => {
                let name = text(req, "name")?;
                if req.get("python").and_then(Value::as_bool) == Some(true) {
                    let src = fs.read_python(name).map_err(e)?;
                    return Ok(Value::object()
                        .with("auto_import", src.auto_import)
                        .with("text", src.text));
                }
                let body = fs.read(name).map_err(e)?;
                Ok(Value::object()
                    .with("size", body.len())
                    .with("content", json::base64_encode(&body)))
            }
            "fs_write" => {
                let name = text(req, "name")?;
                if req.get("python").and_then(Value::as_bool) == Some(true) {
                    let source = PythonSource {
                        auto_import: req
                            .get("auto_import")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        text: text(req, "text")?.to_owned(),
                    };
                    fs.write_python(name, &source).map_err(e)?;
                } else {
                    fs.write(name, &body_of(req)?).map_err(e)?;
                }
                Ok(Value::object())
            }
            "fs_remove" => {
                fs.remove(text(req, "name")?).map_err(e)?;
                Ok(Value::object())
            }
            "fs_rename" => {
                fs.rename(text(req, "from")?, text(req, "to")?).map_err(e)?;
                Ok(Value::object())
            }
            other => Err(format!("commande inconnue : {other}")),
        }
    }

    fn settings(&mut self, cmd: &str, req: &Value) -> Reply {
        let emu = self.emu()?;
        let mut settings = emu.settings();
        if cmd == "settings_get" {
            let p = settings.profile();
            let s = settings.status();
            return Ok(Value::object()
                .with("model", p.model.name())
                .with("software_version", p.software_version.as_str())
                .with(
                    "expected_userland_version",
                    p.expected_userland_version.as_str(),
                )
                .with("commit_hash", p.commit_hash.as_str())
                .with("security_level", p.security_level)
                .with(
                    "active_slot",
                    if p.active_slot == Slot::A { "a" } else { "b" },
                )
                .with("fcc_id", p.fcc_id.as_str())
                .with("pcb_version", p.pcb_version)
                .with("serial_number", p.serial_number.as_str())
                .with("device_name", p.device_name.as_str())
                .with("brightness", u32::from(s.brightness))
                .with(
                    "battery",
                    Value::object()
                        .with("level", u32::from(s.battery_level))
                        .with("voltage", f64::from(s.battery_voltage))
                        .with("charging", s.battery_charging),
                )
                .with("usb_plugged", s.usb_plugged)
                .with("clearance_level", s.clearance_level)
                .with("compilation_flags", u32::from(s.compilation_flags))
                .with("bootloader_crc", s.bootloader_crc)
                .with("last_reset_type", u32::from(s.last_reset_type))
                .with("exam_mode", settings.exam_mode()));
        }
        if cmd != "settings_set" {
            return Err(format!("commande inconnue : {cmd}"));
        }
        let name = text(req, "name")?;
        let value = req.get("value").ok_or("champ « value » manquant")?;
        let string = || {
            value
                .as_str()
                .ok_or_else(|| format!("« {name} » attend un texte"))
        };
        let int = || {
            value
                .as_u32()
                .ok_or_else(|| format!("« {name} » attend un entier"))
        };
        let flag = || {
            value
                .as_bool()
                .ok_or_else(|| format!("« {name} » attend un booléen"))
        };
        let d = |e: crate::device::DeviceError| e.to_string();
        match name {
            "device_name" => settings.set_device_name(string()?).map_err(d)?,
            "fcc_id" => settings.set_fcc_id(string()?).map_err(d)?,
            "pcb_version" => settings.set_pcb_version(int()?).map_err(d)?,
            "serial_number" => settings.set_serial_number(string()?).map_err(d)?,
            "version" => settings.set_version(string()?).map_err(d)?,
            "software_version" => settings.set_software_version(string()?).map_err(d)?,
            "expected_userland_version" => {
                settings
                    .set_expected_userland_version(string()?)
                    .map_err(d)?;
            }
            "commit_hash" => settings.set_commit_hash(string()?).map_err(d)?,
            "security_level" => settings.set_security_level(int()?).map_err(d)?,
            "active_slot" => {
                let slot = match string()? {
                    "a" | "A" => Slot::A,
                    "b" | "B" => Slot::B,
                    other => return Err(format!("slot inconnu : {other} (a ou b)")),
                };
                settings.set_active_slot(slot).map_err(d)?;
            }
            "brightness" => settings.set_brightness(int()?.min(255) as u8),
            "battery" => {
                let level = value
                    .get("level")
                    .and_then(Value::as_u32)
                    .ok_or("battery.level manquant")?;
                let volts = value
                    .get("voltage")
                    .and_then(Value::as_f64)
                    .ok_or("battery.voltage manquant")?;
                let charging = value
                    .get("charging")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                settings.set_battery(level.min(255) as u8, volts as f32, charging);
            }
            "usb_plugged" => settings.set_usb_plugged(flag()?),
            "clearance_level" => settings.set_clearance_level(int()?),
            "compilation_flags" => settings.set_compilation_flags(int()?.min(0xffff) as u16),
            "bootloader_crc" => settings.set_bootloader_crc(int()?),
            "last_reset_type" => settings.set_last_reset_type(int()?.min(255) as u8),
            "exam_mode" => settings.set_exam_mode(flag()?),
            other => return Err(format!("réglage inconnu : {other}")),
        }
        Ok(Value::object())
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
