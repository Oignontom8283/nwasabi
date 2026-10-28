//! `.nds` — Nwasabi Dump State : l'état d'une émulation en pause, pour la reprendre plus tard.
//!
//! Le document est un objet JSON dont les parties sont facultatives (voir `docs/format/nds.md`) :
//!
//! - `device` (toujours) : modèle, disposition du firmware et réglages du profil modifiés ;
//! - `app` : le `.nwa` et ses données externes ;
//! - `memory` : les pages de mémoire qui diffèrent de celles d'un appareil neuf (avec le même `.nwa`), et
//!   les indicateurs d'accès ;
//! - `fs` : les fichiers de la calculatrice, au format `.nfs` ;
//! - `cpu` et `kernel` : l'état d'exécution (registres, temps, écran, clavier…).
//!
//! Enregistrer seulement ce qui diffère d'un appareil neuf garde le fichier petit : une application qui n'a
//! touché que quelques pages de RAM donne quelques ko, pas les 256 Kio de la SRAM.

use super::{base64, boolean, field, nfs, number, number32, text};
use crate::cpu::Cpu;
use crate::emulator::{AppSource, Emulator};
use crate::formats::nfd;
use crate::json::{self, Value, ValueExt};
use crate::kernel::KernelConfig;
use crate::memory::PAGE_SIZE;
use crate::profile::{FirmwareProfile, Model, Slot};

/// Version du format.
pub const VERSION: u64 = 1;

/// Ce que l'on enregistre. Tout est vrai par défaut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parts {
    pub app: bool,
    pub memory: bool,
    pub fs: bool,
    /// Registres du processeur, temps, écran, clavier : l'état d'exécution.
    pub state: bool,
}

impl Default for Parts {
    fn default() -> Self {
        Self {
            app: true,
            memory: true,
            fs: true,
            state: true,
        }
    }
}

/// Un appareil rétabli, avec ce que le document contenait.
pub struct Restored {
    pub emulator: Emulator,
    /// Disposition du firmware (`"24"` ou `"26.4.0"`).
    pub firmware: String,
    /// Les parties que le document contenait, parmi `app`, `memory`, `fs`, `state`.
    pub parts: Vec<&'static str>,
}

fn model_of(name: &str) -> Result<Model, String> {
    Model::ALL
        .into_iter()
        .find(|m| m.name().eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("modèle inconnu : {name}"))
}

/// Les réglages du profil que l'hôte peut changer (voir `Settings`).
fn profile_overrides(profile: &FirmwareProfile, exam_mode: bool) -> Value {
    Value::object()
        .with("software_version", profile.software_version.as_str())
        .with(
            "expected_userland_version",
            profile.expected_userland_version.as_str(),
        )
        .with("commit_hash", profile.commit_hash.as_str())
        .with("security_level", profile.security_level)
        .with(
            "active_slot",
            if profile.active_slot == Slot::A {
                "a"
            } else {
                "b"
            },
        )
        .with("fcc_id", profile.fcc_id.as_str())
        .with("pcb_version", profile.pcb_version)
        .with("serial_number", profile.serial_number.as_str())
        .with("device_name", profile.device_name.as_str())
        .with("exam_mode", exam_mode)
}

fn apply_overrides(emulator: &mut Emulator, device: &Value) -> Result<(), String> {
    let get = |name: &str| text(device, name).map(str::to_owned);
    let (software, userland, hash) = (
        get("software_version")?,
        get("expected_userland_version")?,
        get("commit_hash")?,
    );
    let (fcc, serial, name) = (get("fcc_id")?, get("serial_number")?, get("device_name")?);
    let security = number32(device, "security_level")?;
    let pcb = number32(device, "pcb_version")?;
    let slot = match text(device, "active_slot")? {
        "a" => Slot::A,
        "b" => Slot::B,
        other => return Err(format!("« active_slot » : a ou b, pas {other:?}")),
    };
    let mut settings = emulator.settings();
    // Un seul changement : les contrôles du profil ne portent que sur le résultat.
    settings
        .change_all(|p| {
            p.software_version = software;
            p.expected_userland_version = userland;
            p.commit_hash = hash;
            p.security_level = security;
            p.active_slot = slot;
            p.fcc_id = fcc;
            p.pcb_version = pcb;
            p.serial_number = serial;
            p.device_name = name;
        })
        .map_err(|e| format!("device : {e}"))
}

/// Un appareil neuf comme celui que décrit `device` (avec son `.nwa` s'il y en a un).
fn fresh(
    model: Model,
    firmware: &str,
    device: &Value,
    app: Option<&AppSource>,
) -> Result<Emulator, String> {
    let profile = FirmwareProfile::for_layout(firmware, model)
        .ok_or_else(|| format!("firmware inconnu : {firmware} (24 ou 26.4.0)"))?;
    let mut emulator =
        Emulator::with_profile(profile, KernelConfig::default()).map_err(|e| e.to_string())?;
    apply_overrides(&mut emulator, device)?;
    if let Some(source) = app {
        emulator
            .install_nwa(&source.nwa, source.data.as_deref())
            .map_err(|e| e.to_string())?;
        if !source.flash.is_empty() {
            emulator
                .install_flash(source.flash.clone())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(emulator)
}

fn cpu_to_json(cpu: &Cpu) -> Value {
    let r = &cpu.regs;
    let words = |a: &[u32]| a.iter().map(|&v| Value::from(v)).collect::<Vec<_>>();
    Value::object()
        .with("r", words(&r.r))
        .with("sp", r.sp)
        .with("lr", r.lr)
        .with("pc", r.pc)
        .with("xpsr", r.xpsr)
        .with("s", words(&r.s))
        .with("fpscr", r.fpscr)
        .with("cycles", cpu.cycles)
        .with("msp", cpu.msp)
        .with("psp", cpu.psp)
        .with("control", cpu.control)
        .with("handler_mode", cpu.handler_mode)
        .with("vtor", cpu.vtor)
        .with("primask", cpu.primask)
        .with("faultmask", cpu.faultmask)
        .with("basepri", cpu.basepri)
        .with("exclusive", cpu.exclusive)
}

fn words<const N: usize>(value: &Value, name: &str) -> Result<[u32; N], String> {
    let list = field(value, name)?
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or_else(|| format!("« {name} » : {N} entiers attendus"))?;
    let mut out = [0u32; N];
    for (slot, v) in out.iter_mut().zip(list) {
        *slot = v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| format!("« {name} » : entier de 32 bits attendu"))?;
    }
    Ok(out)
}

fn cpu_from_json(value: &Value) -> Result<Cpu, String> {
    let mut cpu = Cpu::new();
    cpu.regs.r = words::<13>(value, "r")?;
    cpu.regs.sp = number32(value, "sp")?;
    cpu.regs.lr = number32(value, "lr")?;
    cpu.regs.pc = number32(value, "pc")?;
    cpu.regs.xpsr = number32(value, "xpsr")?;
    cpu.regs.s = words::<32>(value, "s")?;
    cpu.regs.fpscr = number32(value, "fpscr")?;
    cpu.cycles = number(value, "cycles")?;
    cpu.msp = number32(value, "msp")?;
    cpu.psp = number32(value, "psp")?;
    cpu.control = number32(value, "control")?;
    cpu.handler_mode = boolean(value, "handler_mode")?;
    cpu.vtor = number32(value, "vtor")?;
    cpu.primask = boolean(value, "primask")?;
    cpu.faultmask = boolean(value, "faultmask")?;
    cpu.basepri =
        u8::try_from(number32(value, "basepri")?).map_err(|_| "« basepri » : trop grand")?;
    cpu.exclusive = match field(value, "exclusive")? {
        Value::Null => None,
        v => Some(
            v.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("« exclusive » : adresse ou null")?,
        ),
    };
    Ok(cpu)
}

/// Enregistre l'état de `emulator` (qui doit être en pause : c'est à l'hôte de le garantir).
pub fn dump(emulator: &mut Emulator, firmware: &str, parts: Parts) -> Result<Value, String> {
    let model = emulator.device.profile.model;
    let exam = emulator.device.exam_mode;
    let device = profile_overrides(&emulator.device.profile, exam)
        .with("model", model.name())
        .with("firmware", firmware);
    let source = emulator.app_source().cloned();
    let mut out = Value::object().with("nds", VERSION).with(
        "created_by",
        format!("nwasabi {}", env!("CARGO_PKG_VERSION")),
    );

    if parts.app
        && let (Some(src), Some(app)) = (&source, emulator.app())
    {
        let mut section = Value::object()
            .with("name", app.name.as_str())
            .with("nwa", json::base64_encode(&src.nwa));
        if let Some(data) = &src.data {
            section.set("data", json::base64_encode(data));
        }
        if !src.flash.is_empty() {
            section.set("flash", nfd::export(&src.flash));
        }
        out.set("app", section);
    }

    if parts.memory {
        let base_app = if parts.app { source.as_ref() } else { None };
        let mut base = fresh(model, firmware, &device, base_app)?;
        let (fs_start, fs_end) = emulator.storage_range();
        let mut runs: Vec<(u32, Vec<u8>)> = Vec::new();
        for address in emulator.device.memory.allocated_page_addresses() {
            let mut now = emulator
                .device
                .memory
                .read_bytes(address, PAGE_SIZE)
                .map_err(|e| e.to_string())?;
            let was = base
                .device
                .memory
                .read_bytes(address, PAGE_SIZE)
                .map_err(|e| e.to_string())?;
            // Les fichiers ont leur propre partie : ils ne comptent pas dans la mémoire.
            let (lo, hi) = (fs_start.max(address), fs_end.min(address + PAGE_SIZE));
            if lo < hi {
                let (a, b) = ((lo - address) as usize, (hi - address) as usize);
                now[a..b].copy_from_slice(&was[a..b]);
            }
            if now == was {
                continue;
            }
            match runs.last_mut() {
                Some((start, bytes)) if *start + bytes.len() as u32 == address => {
                    bytes.extend_from_slice(&now);
                }
                _ => runs.push((address, now)),
            }
        }
        let pages: Vec<Value> = runs
            .iter()
            .map(|(address, bytes)| {
                Value::object()
                    .with("address", *address)
                    .with("content", json::base64_encode(bytes))
            })
            .collect();
        let memory = &emulator.device.memory;
        let access: Vec<Value> = memory
            .regions()
            .iter()
            .filter_map(|region| {
                let runs = access_runs(memory.access_map(&region.name)?);
                (!runs.is_empty()).then(|| {
                    Value::object()
                        .with("region", region.name.as_str())
                        .with("runs", runs)
                })
            })
            .collect();
        out.set(
            "memory",
            Value::object().with("pages", pages).with("access", access),
        );
    }

    if parts.fs {
        out.set("fs", nfs::export(&mut emulator.fs())?);
    }
    if parts.state {
        out.set("cpu", cpu_to_json(&emulator.cpu));
        out.set("kernel", emulator.kernel.to_state());
    }
    out.set("device", device);
    Ok(out)
}

/// Ce que l'appareil actuel garde quand le document n'en parle pas : son `.nwa` et ses fichiers.
pub struct Current {
    pub app: Option<AppSource>,
    /// Ses fichiers, au format `.nfs`.
    pub fs: Option<Value>,
}

/// Relève ce qu'un appareil garde d'une restauration qui ne le remplace pas (voir [`restore`]).
pub fn current(emulator: &mut Emulator) -> Current {
    Current {
        app: emulator.app_source().cloned(),
        fs: nfs::export(&mut emulator.fs()).ok(),
    }
}

/// Rétablit un appareil depuis un document. Ce que le document ne contient pas (`.nwa`, fichiers) est repris
/// de l'appareil actuel (`current`) ; le reste repart d'un appareil neuf.
pub fn restore(document: &Value, current: Option<&Current>) -> Result<Restored, String> {
    let fallback_app = current.and_then(|c| c.app.as_ref());
    let version = number(document, "nds")
        .map_err(|_| "ce n'est pas un fichier .nds (champ « nds » manquant)".to_owned())?;
    if version != VERSION {
        return Err(format!(
            "version {version} non prise en charge (seule {VERSION} l'est)"
        ));
    }
    let device = field(document, "device")?;
    let model = model_of(text(device, "model").map_err(|e| format!("device : {e}"))?)?;
    let firmware = text(device, "firmware")
        .map_err(|e| format!("device : {e}"))?
        .to_owned();
    let mut parts = Vec::new();

    let app = match document.get("app") {
        Some(section) => {
            parts.push("app");
            Some(AppSource {
                nwa: base64(section, "nwa").map_err(|e| format!("app : {e}"))?,
                data: match section.get("data") {
                    Some(_) => Some(base64(section, "data").map_err(|e| format!("app : {e}"))?),
                    None => None,
                },
                flash: match section.get("flash") {
                    Some(doc) => nfd::parse(doc).map_err(|e| format!("app : flash : {e}"))?,
                    None => Vec::new(),
                },
            })
        }
        None => fallback_app.cloned(),
    };
    let mut emulator = fresh(model, &firmware, device, app.as_ref())?;
    if let Some(memory) = document.get("memory") {
        parts.push("memory");
        let pages = field(memory, "pages")?
            .as_array()
            .ok_or("memory : « pages » : une liste est attendue")?;
        for (i, page) in pages.iter().enumerate() {
            let at = |m: String| format!("memory.pages[{i}] : {m}");
            let address = number32(page, "address").map_err(at)?;
            let bytes = base64(page, "content").map_err(at)?;
            if address % PAGE_SIZE != 0 || bytes.len() % PAGE_SIZE as usize != 0 {
                return Err(at(format!(
                    "adresse et taille doivent être des multiples de {PAGE_SIZE}"
                )));
            }
            emulator
                .device
                .memory
                .write_bytes(address, &bytes)
                .map_err(|e| at(e.to_string()))?;
        }
        for (i, entry) in field(memory, "access")?
            .as_array()
            .ok_or("memory : « access » : une liste est attendue")?
            .iter()
            .enumerate()
        {
            let at = |m: String| format!("memory.access[{i}] : {m}");
            let name = text(entry, "region").map_err(at)?;
            let mut map = emulator
                .device
                .memory
                .access_map(name)
                .ok_or_else(|| at(format!("zone inconnue : {name}")))?
                .to_vec();
            map.fill(0);
            for (j, run) in field(entry, "runs")
                .map_err(at)?
                .as_array()
                .ok_or_else(|| at("« runs » : une liste est attendue".into()))?
                .iter()
                .enumerate()
            {
                let at = |m: String| format!("memory.access[{i}].runs[{j}] : {m}");
                let line = number32(run, "line").map_err(at)? as usize;
                let bytes = base64(run, "flags").map_err(at)?;
                let end = line + bytes.len();
                if end > map.len() {
                    return Err(at("hors de la zone".into()));
                }
                map[line..end].copy_from_slice(&bytes);
            }
            emulator.device.memory.set_access_map(name, &map);
        }
    }

    // Les fichiers que le document ne contient pas restent ceux de l'appareil actuel.
    match (document.get("fs"), current.and_then(|c| c.fs.as_ref())) {
        (Some(files), _) => {
            parts.push("fs");
            nfs::import(&mut emulator.fs(), files, true).map_err(|e| format!("fs : {e}"))?;
        }
        (None, Some(files)) => {
            nfs::import(&mut emulator.fs(), files, true).map_err(|e| format!("fs : {e}"))?;
        }
        (None, None) => {}
    }

    match (document.get("cpu"), document.get("kernel")) {
        (Some(cpu), Some(kernel)) => {
            parts.push("state");
            emulator.cpu = cpu_from_json(cpu).map_err(|e| format!("cpu : {e}"))?;
            emulator
                .kernel
                .restore_state(kernel)
                .map_err(|e| format!("kernel : {e}"))?;
        }
        (None, None) => {}
        _ => return Err("« cpu » et « kernel » vont ensemble".into()),
    }

    if let Ok(exam) = boolean(device, "exam_mode") {
        emulator.device.exam_mode = exam;
    }
    Ok(Restored {
        emulator,
        firmware,
        parts,
    })
}

/// Les stretches d'indicateurs d'accès non nuls (en fusionnant ceux que moins de 16 lignes séparent), en
/// `{ line, flags }` : une zone de plusieurs Mio où le programme n'a touché que quelques lignes tient en
/// quelques octets.
fn access_runs(map: &[u8]) -> Vec<Value> {
    const GAP: usize = 16;
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (line, _) in map.iter().enumerate().filter(|(_, b)| **b != 0) {
        match runs.last_mut() {
            Some((_, end)) if line < *end + GAP => *end = line + 1,
            _ => runs.push((line, line + 1)),
        }
    }
    runs.into_iter()
        .map(|(start, end)| {
            Value::object()
                .with("line", start)
                .with("flags", json::base64_encode(&map[start..end]))
        })
        .collect()
}
