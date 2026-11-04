//! L'exécution : lancer, avancer le temps, appuyer sur une touche, pas à pas, points d'arrêt, registres écrits, profil et pile.

use super::{
    CHUNK, Reply, Session, apply_speed, debug, key_of, number, register_name, register_number,
    text, with_speed,
};
use crate::checksum;
use crate::emulator::RunStop;
use crate::json;
use crate::json::Value;
use crate::json::ValueExt;
use crate::kernel::ClockMode;

/// Budget d'instructions par défaut d'une commande `run`.
const DEFAULT_INSTRUCTIONS: u64 = 10_000_000;

impl Session {
    /// Exécute jusqu'à un arrêt qui demande l'hôte, `frames` images ou `instructions` instructions.
    pub(super) fn run(&mut self, req: &Value) -> Reply {
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

    pub(super) fn key(&mut self, req: &Value) -> Reply {
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

    pub(super) fn screen(&mut self, req: &Value) -> Reply {
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

    /// La commande `step`.
    pub(super) fn step(&mut self) -> Reply {
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

    /// La commande `break`.
    pub(super) fn break_at(&mut self, req: &Value) -> Reply {
        let address = number(req, "address")?;
        // Condition facultative : { "register": "r0", "op": "==", "value": 5 }.
        let condition = match req.get("condition") {
            Some(c) if !matches!(c, Value::Null) => {
                let name = text(c, "register")?.to_ascii_lowercase();
                let register =
                    register_number(&name).ok_or_else(|| format!("registre inconnu : {name}"))?;
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

    /// La commande `unbreak`.
    pub(super) fn unbreak(&mut self, req: &Value) -> Reply {
        let address = number(req, "address")?;
        let removed = self.emu()?.remove_breakpoint(address);
        Ok(Value::object().with("removed", removed))
    }

    /// La commande `breaks`.
    pub(super) fn breaks(&mut self) -> Reply {
        Ok(Value::object()
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
            ))
    }

    /// La commande `cpu_set`.
    pub(super) fn cpu_set(&mut self, req: &Value) -> Reply {
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

    /// La commande `advance`.
    pub(super) fn advance(&mut self, req: &Value) -> Reply {
        // `us` (microsecondes) ou `ms` : le temps à faire passer pour l'application.
        let us = match req.get("us") {
            Some(_) => u64::from(number(req, "us")?),
            None => u64::from(number(req, "ms")?) * 1000,
        };
        self.emu()?.kernel.advance_time(us);
        Ok(Value::object().with("time_ms", self.time_ms()?))
    }

    /// La commande `speed`.
    pub(super) fn speed(&mut self, req: &Value) -> Reply {
        // Change la vitesse du processeur (et/ou la fréquence personnalisée) ; sans champ, la rend.
        let emu = self.emu()?;
        apply_speed(req, &mut emu.kernel.config)?;
        Ok(with_speed(Value::object(), &emu.kernel))
    }

    /// La commande `time`.
    pub(super) fn time(&mut self) -> Reply {
        // Temps de simulation depuis le début : celui de l'application, lisible à tout moment.
        let us = self.emu()?.time_us();
        Ok(Value::object()
            .with("time_us", us)
            .with("time_ms", us / 1000))
    }

    /// La commande `stdout`.
    pub(super) fn stdout(&mut self, req: &Value) -> Reply {
        let emu = self.emu()?;
        let out = String::from_utf8_lossy(&emu.kernel.stdout).into_owned();
        if req.get("clear").and_then(Value::as_bool) == Some(true) {
            emu.kernel.stdout.clear();
        }
        Ok(Value::object().with("text", out))
    }

    /// La commande `profile`.
    pub(super) fn profile(&mut self, req: &Value) -> Reply {
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

    /// La commande `backtrace`.
    pub(super) fn backtrace(&mut self, req: &Value) -> Reply {
        let max = req
            .get("max")
            .and_then(Value::as_u32)
            .unwrap_or(32)
            .min(256) as usize;
        Ok(debug::backtrace(self.emu()?, max))
    }
}
