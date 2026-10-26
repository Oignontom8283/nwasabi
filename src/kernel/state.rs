//! Enregistrement et restauration de l'état du noyau (voir `snapshot` pour le format `.nds`).
//!
//! Tout ce qui fait l'état d'une exécution en pause y passe : écran, clavier (touches tenues, événements en
//! attente, répétition), temps, compteurs, aléa, tas, attente en cours, réglages de l'appareil. Seul l'arrêt
//! demandé au noyau (`stop`) n'y est pas : il est toujours consommé avant que l'hôte reprenne la main.

use super::display::{HEIGHT, WIDTH};
use super::{ClockMode, CpuSpeed, EventWait, Kernel};
use crate::json::{self, Value, ValueExt};

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, String> {
    value
        .get(name)
        .ok_or_else(|| format!("champ « {name} » manquant"))
}

fn uint(value: &Value, name: &str) -> Result<u64, String> {
    field(value, name)?
        .as_u64()
        .ok_or_else(|| format!("« {name} » : un entier positif est attendu"))
}

fn small(value: &Value, name: &str) -> Result<u32, String> {
    u32::try_from(uint(value, name)?).map_err(|_| format!("« {name} » : valeur trop grande"))
}

fn flag(value: &Value, name: &str) -> Result<bool, String> {
    field(value, name)?
        .as_bool()
        .ok_or_else(|| format!("« {name} » : vrai ou faux est attendu"))
}

/// Les pixels en deux octets chacun (`raw16`), ou en suites de pixels identiques (`rle16` : un nombre de
/// répétitions sur deux octets puis le pixel, tous little-endian), le plus court des deux.
fn encode_display(pixels: &[u16]) -> Value {
    let raw: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
    let mut rle = Vec::new();
    let mut i = 0;
    while i < pixels.len() {
        let color = pixels[i];
        let mut run = 1;
        while i + run < pixels.len() && pixels[i + run] == color && run < usize::from(u16::MAX) {
            run += 1;
        }
        rle.extend_from_slice(&(run as u16).to_le_bytes());
        rle.extend_from_slice(&color.to_le_bytes());
        i += run;
    }
    let (encoding, bytes) = if rle.len() < raw.len() {
        ("rle16", rle)
    } else {
        ("raw16", raw)
    };
    Value::object()
        .with("encoding", encoding)
        .with("base64", json::base64_encode(&bytes))
}

fn decode_display(v: &Value) -> Result<Vec<u16>, String> {
    let bytes = json::base64_decode(
        field(v, "base64")?
            .as_str()
            .ok_or("« display.base64 » : texte attendu")?,
    )
    .ok_or("« display.base64 » : base64 invalide")?;
    let expected = (WIDTH * HEIGHT) as usize;
    let pixels: Vec<u16> = match field(v, "encoding")?.as_str() {
        Some("raw16") => bytes
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect(),
        Some("rle16") => {
            let mut out = Vec::with_capacity(expected);
            for run in bytes.chunks_exact(4) {
                let count = usize::from(u16::from_le_bytes([run[0], run[1]]));
                if out.len() + count > expected {
                    return Err("« display » : trop de pixels".into());
                }
                out.resize(out.len() + count, u16::from_le_bytes([run[2], run[3]]));
            }
            out
        }
        _ => return Err("« display.encoding » : raw16 ou rle16".into()),
    };
    if pixels.len() != expected {
        return Err(format!(
            "« display » : {} pixels au lieu de {expected}",
            pixels.len()
        ));
    }
    Ok(pixels)
}

impl Kernel {
    /// L'état du noyau, en JSON.
    pub fn to_state(&self) -> Value {
        let c = &self.config;
        let s = &self.status;
        let display = encode_display(self.display.pixels());
        let wait = self.wait.map(|w| {
            Value::object()
                .with("started_us", w.started_us)
                .with("timeout_ms", w.timeout_ms)
        });
        Value::object()
            .with(
                "config",
                Value::object()
                    .with(
                        "clock",
                        match c.clock {
                            ClockMode::Virtual => "virtual",
                            ClockMode::Host => "host",
                        },
                    )
                    .with(
                        "speed",
                        match c.speed {
                            CpuSpeed::Model => "model",
                            CpuSpeed::Custom => "custom",
                            CpuSpeed::Max => "max",
                        },
                    )
                    .with("mhz", c.custom_mhz)
                    .with("strict_svc", c.strict_svc)
                    .with("stdout_to_screen", c.stdout_to_screen)
                    .with("random_seed", c.random_seed),
            )
            .with(
                "status",
                Value::object()
                    .with("brightness", s.brightness)
                    .with("battery_level", s.battery_level)
                    .with("battery_voltage", f64::from(s.battery_voltage))
                    .with("battery_charging", s.battery_charging)
                    .with("usb_plugged", s.usb_plugged)
                    .with("clearance_level", s.clearance_level)
                    .with("pcb_version", s.pcb_version)
                    .with("serial_number", s.serial_number.as_str())
                    .with("fcc_id_address", s.fcc_id_address)
                    .with("compilation_flags", s.compilation_flags)
                    .with("bootloader_crc", s.bootloader_crc)
                    .with("last_reset_type", s.last_reset_type)
                    .with("cpu_mhz", s.cpu_mhz),
            )
            .with("display", display)
            .with("keyboard", self.keyboard.to_state())
            .with("stdout_base64", json::base64_encode(&self.stdout))
            .with("frames", self.frames)
            .with("unknown_svcs", self.unknown_svcs)
            .with(
                "svc_counts",
                self.svc_counts
                    .iter()
                    .map(|&n| Value::from(n))
                    .collect::<Vec<_>>(),
            )
            .with("extra_us", self.extra_us)
            .with("sleep_until_us", self.sleep_until_us)
            .with("repeat_since_us", self.repeat_since_us)
            .with("repeat_count", self.repeat_count)
            .with("random", self.random)
            .with("heap", vec![self.heap.0, self.heap.1])
            .with("brk", self.brk)
            .with("wait", wait.unwrap_or(Value::Null))
    }

    /// Remplace l'état du noyau par un état enregistré. En cas d'erreur, le noyau n'est pas modifié.
    pub fn restore_state(&mut self, v: &Value) -> Result<(), String> {
        let mut next = self.clone();
        let c = field(v, "config")?;
        next.config.clock = match field(c, "clock")?.as_str() {
            Some("virtual") => ClockMode::Virtual,
            Some("host") => ClockMode::Host,
            _ => return Err("« config.clock » : virtual ou host".into()),
        };
        next.config.speed = match field(c, "speed")?.as_str() {
            Some("model") => CpuSpeed::Model,
            Some("custom") => CpuSpeed::Custom,
            Some("max") => CpuSpeed::Max,
            _ => return Err("« config.speed » : model, custom ou max".into()),
        };
        next.config.custom_mhz = small(c, "mhz")?.max(1);
        next.config.strict_svc = flag(c, "strict_svc")?;
        next.config.stdout_to_screen = flag(c, "stdout_to_screen")?;
        next.config.random_seed = small(c, "random_seed")?;

        let s = field(v, "status")?;
        next.status.brightness = u8::try_from(small(s, "brightness")?).map_err(|_| "brightness")?;
        next.status.battery_level =
            u8::try_from(small(s, "battery_level")?).map_err(|_| "battery_level")?;
        next.status.battery_voltage = field(s, "battery_voltage")?
            .as_f64()
            .ok_or("« status.battery_voltage » : un nombre est attendu")?
            as f32;
        next.status.battery_charging = flag(s, "battery_charging")?;
        next.status.usb_plugged = flag(s, "usb_plugged")?;
        next.status.clearance_level = small(s, "clearance_level")?;
        next.status.pcb_version = small(s, "pcb_version")?;
        next.status.serial_number = field(s, "serial_number")?
            .as_str()
            .ok_or("« status.serial_number » : texte attendu")?
            .to_owned();
        next.status.fcc_id_address = small(s, "fcc_id_address")?;
        next.status.compilation_flags =
            u16::try_from(small(s, "compilation_flags")?).map_err(|_| "compilation_flags")?;
        next.status.bootloader_crc = small(s, "bootloader_crc")?;
        next.status.last_reset_type =
            u8::try_from(small(s, "last_reset_type")?).map_err(|_| "last_reset_type")?;
        next.status.cpu_mhz = small(s, "cpu_mhz")?.max(1);

        let pixels = decode_display(field(v, "display")?)?;
        for (i, &color) in pixels.iter().enumerate() {
            next.display.set(i as u32 % WIDTH, i as u32 / WIDTH, color);
        }
        next.keyboard.restore_state(field(v, "keyboard")?)?;
        next.stdout = json::base64_decode(
            field(v, "stdout_base64")?
                .as_str()
                .ok_or("« stdout_base64 » : texte attendu")?,
        )
        .ok_or("« stdout_base64 » : base64 invalide")?;
        next.frames = uint(v, "frames")?;
        next.unknown_svcs = small(v, "unknown_svcs")?;
        let counts = field(v, "svc_counts")?
            .as_array()
            .ok_or("« svc_counts » : une liste est attendue")?;
        if counts.len() != next.svc_counts.len() {
            return Err("« svc_counts » : taille inattendue".into());
        }
        for (slot, n) in next.svc_counts.iter_mut().zip(counts) {
            *slot = n
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("« svc_counts » : entier invalide")?;
        }
        next.extra_us = uint(v, "extra_us")?;
        let opt = |name: &str| -> Result<Option<u64>, String> {
            match field(v, name)? {
                Value::Null => Ok(None),
                n => n
                    .as_u64()
                    .map(Some)
                    .ok_or(format!("« {name} » : entier ou null")),
            }
        };
        next.sleep_until_us = opt("sleep_until_us")?;
        next.repeat_since_us = opt("repeat_since_us")?;
        next.repeat_count = small(v, "repeat_count")?;
        next.random = small(v, "random")?;
        let heap = field(v, "heap")?
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or("« heap » : deux adresses attendues")?;
        let addr = |i: usize| {
            heap[i]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("« heap » : adresse invalide")
        };
        next.heap = (addr(0)?, addr(1)?);
        next.brk = small(v, "brk")?;
        next.wait = match field(v, "wait")? {
            Value::Null => None,
            w => Some(EventWait {
                started_us: uint(w, "started_us")?,
                timeout_ms: i32::try_from(
                    field(w, "timeout_ms")?
                        .as_i64()
                        .ok_or("« wait.timeout_ms » : entier attendu")?,
                )
                .map_err(|_| "« wait.timeout_ms » : valeur trop grande")?,
            }),
        };
        next.stop = None;
        *self = next;
        Ok(())
    }
}
