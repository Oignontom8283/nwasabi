//! Débogage en remontant le temps : l'émulation étant déterministe, on garde des points de reprise réguliers (processeur, noyau,
//! mémoire vive) et le journal des touches et des passages de temps donnés par l'hôte ; revenir en arrière, c'est rétablir le
//! point de reprise précédent puis rejouer, instruction après instruction, jusqu'à l'endroit voulu.
//!
//! Tant qu'un historique tourne, toute commande qui modifie l'appareil autrement que par l'exécution, les touches ou le temps
//! (écrire en mémoire, changer un registre, charger…) le remet à zéro : on ne peut pas rejouer ce qui n'a pas été enregistré.

use std::collections::VecDeque;

use crate::cpu::Cpu;
use crate::emulator::{Emulator, RunStop};
use crate::json::{Value, ValueExt};
use crate::kernel::Kernel;
use crate::memory::VolatileImage;

use super::{Reply, Session, views};

/// Instructions entre deux points de reprise, et nombre de points gardés : l'historique couvre `interval × capacity` instructions.
const DEFAULT_INTERVAL: u64 = 250_000;
const DEFAULT_CAPACITY: usize = 100;
/// Au plus ce nombre de tours de rejeu sans progrès avant d'abandonner (évite de tourner en rond).
const STALL_LIMIT: u32 = 100_000;

struct Checkpoint {
    cycles: u64,
    cpu: Cpu,
    kernel: Kernel,
    memory: VolatileImage,
    frames: u64,
}

/// Une commande de l'hôte qui change le déroulement : touche ou passage de temps, à rejouer.
struct Input {
    cycles: u64,
    request: Value,
}

pub struct History {
    interval: u64,
    capacity: usize,
    checkpoints: VecDeque<Checkpoint>,
    inputs: Vec<Input>,
    /// Compteur de modifications de la mémoire à la dernière vérification de la flash.
    flash_cursor: u64,
}

/// Ce qu'une commande fait à l'historique.
pub enum Effect {
    /// Rien : lecture, point d'arrêt…
    None,
    /// À enregistrer pour le rejeu (touche, temps).
    Input,
    /// Fait avancer l'exécution (traité par `run` et `step` eux-mêmes).
    Forward,
    /// Modifie l'appareil sans être rejouable : l'historique repart de zéro.
    Reset,
}

/// Classe une commande selon son effet sur l'historique.
pub fn effect_of(cmd: &str) -> Effect {
    match cmd {
        "key" | "advance" => Effect::Input,
        "run" | "step" | "step_source" | "step_back" | "reverse_continue" | "history_goto" => {
            Effect::Forward
        }
        "cpu" | "regions" | "usage" | "memory_read" | "changes" | "watch" | "unwatch"
        | "watch_list" | "watch_hits" | "svc_stats" | "svc_log" | "time" | "screen" | "inspect"
        | "disasm" | "symbol" | "symbols" | "backtrace" | "profile" | "break" | "unbreak"
        | "breaks" | "fs_list" | "fs_stats" | "fs_read" | "fs_export" | "settings_get" | "dump"
        | "help" | "history" | "stdout" | "flash_export" => Effect::None,
        // Le débogage du code source : de la lecture, ou des expressions qui signalent elles-mêmes qu'elles ont modifié l'appareil
        // (affectation, appel de fonction) et remettent alors l'historique à zéro.
        "debug_info" | "line_at" | "source_lines" | "break_line" | "unbreak_line"
        | "break_function" | "frames" | "locals" | "globals" | "eval" | "assign" | "ptype"
        | "export_elf" => Effect::None,
        _ => Effect::Reset,
    }
}

impl History {
    fn new(interval: u64, capacity: usize, emu: &Emulator) -> Self {
        Self {
            interval: interval.max(1_000),
            capacity: capacity.clamp(2, 2_000),
            checkpoints: VecDeque::new(),
            inputs: Vec::new(),
            flash_cursor: emu.device.memory.change_cursor(),
        }
    }

    fn take(&mut self, emu: &Emulator, frames: u64) {
        // La flash qui change (fichiers écrits par l'application) ne se rétablit pas : ce qui précède n'est plus rejouable.
        if emu.device.memory.flash_changed_since(self.flash_cursor) {
            self.checkpoints.clear();
            self.inputs.clear();
        }
        self.flash_cursor = emu.device.memory.change_cursor();
        self.checkpoints.push_back(Checkpoint {
            cycles: emu.cpu.cycles,
            cpu: emu.cpu.clone(),
            kernel: emu.kernel.clone(),
            memory: emu.device.memory.snapshot_volatile(),
            frames,
        });
        while self.checkpoints.len() > self.capacity {
            self.checkpoints.pop_front();
            // Les touches d'avant le plus ancien point de reprise ne servent plus.
            let oldest = self.checkpoints.front().map_or(0, |c| c.cycles);
            self.inputs.retain(|i| i.cycles >= oldest);
        }
    }

    /// Faut-il un nouveau point de reprise ?
    pub fn due(&self, cycles: u64) -> bool {
        self.checkpoints
            .back()
            .is_none_or(|c| cycles >= c.cycles + self.interval)
    }

    pub fn interval(&self) -> u64 {
        self.interval
    }
}

impl Session {
    /// Prend un point de reprise si l'historique tourne et que c'est l'heure.
    pub(super) fn history_tick(&mut self) {
        let frames = self.frames;
        if let (Some(history), Some(emu)) = (self.history.as_mut(), self.emu.as_ref())
            && history.due(emu.cpu.cycles)
        {
            history.take(emu, frames);
        }
    }

    /// Instructions à ne pas dépasser pour la prochaine tranche d'exécution (afin de ne pas manquer l'heure d'un point de reprise).
    pub(super) fn history_slice(&self) -> Option<u64> {
        self.history.as_ref().map(History::interval)
    }

    /// Note une touche ou un passage de temps de l'hôte, pour pouvoir le rejouer.
    pub(super) fn history_record(&mut self, request: &Value) {
        self.history_fork();
        if let (Some(history), Some(emu)) = (self.history.as_mut(), self.emu.as_ref()) {
            history.inputs.push(Input {
                cycles: emu.cpu.cycles,
                request: request.clone(),
            });
        }
    }

    /// L'appareil vient d'être modifié autrement : l'historique repart de zéro (s'il tourne).
    pub(super) fn history_reset(&mut self) {
        let frames = self.frames;
        if let (Some(history), Some(emu)) = (self.history.as_mut(), self.emu.as_ref()) {
            history.checkpoints.clear();
            history.inputs.clear();
            history.take(emu, frames);
        }
    }

    /// Rétablit le point de reprise le plus récent à `target` instructions ou avant, sans toucher aux touches enregistrées.
    fn restore_before(&mut self, target: u64) -> Result<u64, String> {
        let history = self
            .history
            .as_ref()
            .ok_or("l'historique n'est pas lancé (history start)")?;
        let checkpoint = history
            .checkpoints
            .iter()
            .rev()
            .find(|c| c.cycles <= target)
            .ok_or(
                "pas d'historique jusque-là : le plus ancien point de reprise est plus récent",
            )?;
        let (cpu, kernel, memory, frames, cycles) = (
            checkpoint.cpu.clone(),
            checkpoint.kernel.clone(),
            checkpoint.memory.clone(),
            checkpoint.frames,
            checkpoint.cycles,
        );
        let emu = self.emu.as_mut().ok_or("aucun appareil")?;
        emu.cpu = cpu;
        emu.kernel = kernel;
        emu.device.memory.restore_volatile(&memory);
        self.frames = frames;
        Ok(cycles)
    }

    /// Rejoue depuis l'état actuel jusqu'à `target` instructions (touches et temps de l'hôte compris). Les points d'arrêt et
    /// d'observation sont ignorés ; si `scan` est donné, il reçoit chaque arrêt qu'ils auraient provoqué (instructions, nature).
    fn replay_to(
        &mut self,
        target: u64,
        mut scan: Option<&mut Vec<(u64, Value)>>,
    ) -> Result<(), String> {
        let mut applied = 0usize;
        {
            let history = self
                .history
                .as_ref()
                .ok_or("l'historique n'est pas lancé")?;
            let now = self.emu.as_ref().ok_or("aucun appareil")?.cpu.cycles;
            // Les entrées d'avant le point de reprise sont déjà dans son état.
            applied = applied.max(history.inputs.iter().take_while(|i| i.cycles < now).count());
        }
        let mut stalls = 0u32;
        loop {
            let now = self.emu.as_ref().ok_or("aucun appareil")?.cpu.cycles;
            // Les entrées dues maintenant.
            loop {
                let due = {
                    let history = self
                        .history
                        .as_ref()
                        .ok_or("l'historique n'est pas lancé")?;
                    history
                        .inputs
                        .get(applied)
                        .filter(|i| i.cycles <= now)
                        .map(|i| i.request.clone())
                };
                let Some(request) = due else { break };
                self.replay_input(&request)?;
                applied += 1;
                stalls = 0;
            }
            if now >= target {
                return Ok(());
            }
            let limit = {
                let history = self
                    .history
                    .as_ref()
                    .ok_or("l'historique n'est pas lancé")?;
                history
                    .inputs
                    .get(applied)
                    .map_or(target, |i| i.cycles.min(target))
            };
            let emu = self.emu.as_mut().ok_or("aucun appareil")?;
            let stop = emu.run((limit - now).max(1));
            let moved = emu.cpu.cycles != now;
            match &stop {
                RunStop::Budget | RunStop::Frame | RunStop::Sleep { .. } => {}
                RunStop::Breakpoint { .. } | RunStop::Watchpoint(_) => {
                    if let Some(list) = scan.as_deref_mut() {
                        list.push((emu.cpu.cycles, views::stop(&stop)));
                    }
                }
                // Une attente d'événement n'avance que si l'hôte a fait quelque chose à cet instant : sinon, le rejeu est fini.
                RunStop::WaitEvent { .. } => {
                    let pending = self.history.as_ref().is_some_and(|h| {
                        h.inputs.get(applied).is_some_and(|i| {
                            i.cycles <= self.emu.as_ref().map_or(0, |e| e.cpu.cycles)
                        })
                    });
                    if !pending && !moved {
                        return Ok(());
                    }
                }
                _ => return Ok(()),
            }
            stalls = if moved { 0 } else { stalls + 1 };
            if stalls > STALL_LIMIT {
                return Err("le rejeu ne progresse plus".to_owned());
            }
        }
    }

    fn replay_input(&mut self, request: &Value) -> Result<(), String> {
        let cmd = request.get("cmd").and_then(Value::as_str).unwrap_or("");
        match cmd {
            "key" => self.key(request).map(|_| ()),
            "advance" => {
                let us = match request.get("us") {
                    Some(_) => u64::from(super::number(request, "us")?),
                    None => u64::from(super::number(request, "ms")?) * 1000,
                };
                self.emu
                    .as_mut()
                    .ok_or("aucun appareil")?
                    .kernel
                    .advance_time(us);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Avant d'exécuter vers l'avant depuis un instant remonté : ce qui était enregistré après n'aura plus lieu (autre chemin).
    pub(super) fn history_fork(&mut self) {
        let now = self.emu.as_ref().map_or(0, |e| e.cpu.cycles);
        if let Some(history) = self.history.as_mut() {
            history.checkpoints.retain(|c| c.cycles <= now);
            history.inputs.retain(|i| i.cycles <= now);
        }
    }

    fn history_to(&mut self, target: u64) -> Result<(), String> {
        self.restore_before(target)?;
        self.emu
            .as_mut()
            .ok_or("aucun appareil")?
            .suspend_debugging(true);
        let outcome = self.replay_to(target, None);
        let emu = self.emu.as_mut().ok_or("aucun appareil")?;
        emu.suspend_debugging(false);
        outcome
    }

    fn position(&mut self) -> Result<Value, String> {
        let emu = self.emu()?;
        Ok(Value::object()
            .with("cycles", emu.cpu.cycles)
            .with("time_ms", emu.time_us() / 1000))
    }

    /// La commande `history` : lancer, arrêter, ou lire l'état de l'historique.
    pub(super) fn history_command(&mut self, req: &Value) -> Reply {
        let action = req
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("status");
        match action {
            "start" => {
                let interval = req
                    .get("interval")
                    .and_then(Value::as_u32)
                    .map_or(DEFAULT_INTERVAL, u64::from);
                let capacity = req
                    .get("capacity")
                    .and_then(Value::as_u32)
                    .map_or(DEFAULT_CAPACITY, |n| n as usize);
                let frames = self.frames;
                let emu = self
                    .emu
                    .as_ref()
                    .ok_or("aucun appareil : utiliser « new » ou « load »")?;
                let mut history = History::new(interval, capacity, emu);
                history.take(emu, frames);
                self.history = Some(history);
            }
            "stop" => self.history = None,
            "status" => {}
            other => return Err(format!("action inconnue : {other} (start, stop, status)")),
        }
        let now = self.emu.as_ref().map_or(0, |e| e.cpu.cycles);
        Ok(match &self.history {
            Some(h) => Value::object()
                .with("recording", true)
                .with("checkpoints", h.checkpoints.len())
                .with("oldest", h.checkpoints.front().map_or(now, |c| c.cycles))
                .with("newest", h.checkpoints.back().map_or(now, |c| c.cycles))
                .with("current", now)
                .with("interval", h.interval)
                .with(
                    "bytes",
                    h.checkpoints
                        .iter()
                        .map(|c| c.memory.bytes() + 320 * 240 * 2)
                        .sum::<usize>(),
                ),
            None => Value::object().with("recording", false),
        })
    }

    /// `step_back` : une instruction en arrière.
    pub(super) fn step_back(&mut self) -> Reply {
        if self.history.is_none() {
            return Err("l'historique n'est pas lancé (history start)".to_owned());
        }
        let now = self.emu()?.cpu.cycles;
        if now == 0 {
            return Err("déjà au début".to_owned());
        }
        self.history_to(now - 1)?;
        self.position()
    }

    /// `history_goto` : revient à un instant enregistré.
    pub(super) fn history_goto(&mut self, req: &Value) -> Reply {
        let cycles = u64::from(super::number(req, "cycles")?);
        self.history_to(cycles)?;
        self.position()
    }

    /// `reverse_continue` : remonte jusqu'au point d'arrêt ou d'observation précédent, ou au début de l'historique.
    pub(super) fn reverse_continue(&mut self) -> Reply {
        let now = self.emu()?.cpu.cycles;
        let mut upper = now;
        loop {
            // Dernier point de reprise strictement avant `upper`.
            let before = {
                let history = self
                    .history
                    .as_ref()
                    .ok_or("l'historique n'est pas lancé (history start)")?;
                history
                    .checkpoints
                    .iter()
                    .rev()
                    .find(|c| c.cycles < upper)
                    .map(|c| c.cycles)
            };
            let Some(start) = before else {
                // Plus rien avant : on revient au début de l'historique.
                let oldest = self
                    .history
                    .as_ref()
                    .and_then(|h| h.checkpoints.front().map(|c| c.cycles))
                    .unwrap_or(0);
                self.history_to(oldest)?;
                return Ok(self
                    .position()?
                    .with("stop", Value::object().with("kind", "history_start")));
            };
            self.restore_before(start)?;
            let mut hits = Vec::new();
            // Les points sont actifs pendant ce balayage : on veut savoir où ils auraient arrêté l'exécution.
            self.emu
                .as_mut()
                .ok_or("aucun appareil")?
                .suspend_debugging(false);
            self.replay_to(upper - 1, Some(&mut hits))?;
            if let Some((cycles, stop)) = hits.into_iter().rfind(|(c, _)| *c < upper && *c < now) {
                self.history_to(cycles)?;
                if stop.get("kind").and_then(Value::as_str) == Some("breakpoint") {
                    self.emu
                        .as_mut()
                        .ok_or("aucun appareil")?
                        .stopped_at_breakpoint();
                }
                return Ok(self.position()?.with("stop", stop));
            }
            upper = start;
        }
    }
}
