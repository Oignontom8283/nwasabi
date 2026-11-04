//! Le cycle de vie de l'appareil : création, chargement d'une application, lancement, état enregistré, données en flash.

use super::{Reply, Session, config_of, model_of, profile_of, text, views, with_speed};
use crate::emulator::Emulator;
use crate::formats::nds;
use crate::formats::nfd;
use crate::json;
use crate::json::Value;
use crate::json::ValueExt;

impl Session {
    pub(super) fn new_device(&mut self, req: &Value) -> Reply {
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

    pub(super) fn load(&mut self, req: &Value) -> Reply {
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

    /// Enregistre l'état de l'appareil (format `.nds`). `parts` (liste parmi `app`, `memory`, `fs`, `state`)
    /// choisit ce qui est gardé ; par défaut, tout.
    pub(super) fn dump(&mut self, req: &Value) -> Reply {
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
    pub(super) fn restore(&mut self, req: &Value) -> Reply {
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

    pub(super) fn firmware(&self) -> &str {
        if self.firmware.is_empty() {
            "24"
        } else {
            &self.firmware
        }
    }

    pub(super) fn time_ms(&mut self) -> Result<u64, String> {
        let emu = self.emu()?;
        Ok(emu.time_us() / 1000)
    }

    /// La commande `launch`.
    pub(super) fn launch(&mut self) -> Reply {
        let emu = self.emu()?;
        emu.launch().map_err(|e| e.to_string())?;
        Ok(Value::object())
    }

    /// La commande `flash_load`.
    pub(super) fn flash_load(&mut self, req: &Value) -> Reply {
        let doc = req.get("nfd").ok_or("champ « nfd » (document) manquant")?;
        let segments = nfd::parse(doc)?;
        let count = segments.len();
        self.emu()?
            .install_flash(segments)
            .map_err(|e| e.to_string())?;
        Ok(Value::object().with("segments", count))
    }

    /// La commande `flash_export`.
    pub(super) fn flash_export(&mut self) -> Reply {
        let doc = nfd::export(self.emu()?.flash_segments());
        Ok(Value::object().with("nfd", doc))
    }
}
