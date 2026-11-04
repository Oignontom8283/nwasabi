//! Les réglages de l'appareil : nom, identifiants, versions, luminosité, vitesse.

use super::{Reply, Session, text};
use crate::json::Value;
use crate::json::ValueExt;
use crate::profile::Slot;

impl Session {
    pub(super) fn settings(&mut self, cmd: &str, req: &Value) -> Reply {
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
