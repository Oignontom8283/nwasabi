//! L'émulateur : un appareil, un processeur et le noyau simulé, réunis pour exécuter une
//! application `.nwa`.
//!
//! L'exécution est coopérative : [`Emulator::run`] avance l'application jusqu'à un point où
//! l'hôte doit intervenir (image prête, attente d'un événement, fin…) et dit pourquoi elle
//! s'est arrêtée. Le cœur ne dort jamais et ne lit aucune horloge : c'est à l'hôte de faire
//! passer le temps ([`Kernel::advance_time`]).

use std::collections::HashMap;
use std::sync::Arc;

use crate::cpu::{Cpu, Stop};
use crate::debuginfo::DebugInfo;
use crate::device::{DeviceImage, EXIT_STUB_OFFSET};
use crate::elf::SymbolKind;
use crate::formats::nfd::{self, Segment};
use crate::kernel::{DeviceStatus, Kernel, KernelConfig, KernelStop};
use crate::link::{LinkOptions, LinkedApp, Origin, link};
use crate::memory::access;
use crate::profile::{FirmwareProfile, Model};
use crate::settings::Settings;
use crate::storage::{Fs, Storage};

mod arrets;
mod erreur;
mod usage;

pub use arrets::{Breakpoint, Compare, Condition, Fault, RunStop};
pub use erreur::EmulatorError;
pub use usage::{Span, Usage};

/// Nombre d'instructions entre deux relevés du profil d'exécution (environ).
const PROFILE_SLICE: u64 = 509;

/// Un segment en quelques mots, pour un message d'erreur.
fn describe(segment: &Segment) -> String {
    format!(
        "« {} » (0x{:08x}..0x{:08x})",
        segment.name,
        segment.address,
        segment.end()
    )
}

/// Un appareil en train d'exécuter une application.
pub struct Emulator {
    pub device: DeviceImage,
    pub cpu: Cpu,
    pub kernel: Kernel,
    storage: Storage,
    app: Option<LinkedApp>,
    /// Les octets donnés à [`Emulator::install_nwa`] (pour enregistrer l'état, voir `snapshot`).
    source: Option<AppSource>,
    /// Octets placés en flash à part de l'application (voir [`Emulator::install_flash`]).
    flash: Vec<Segment>,
    /// Les informations de débogage de l'application, lues à la première demande (`Some(None)` : l'application n'en a pas).
    debug: Option<Option<Arc<DebugInfo>>>,
    /// Adresses où l'exécution s'arrête avant l'instruction (voir [`Emulator::add_breakpoint`]).
    breakpoints: Vec<Breakpoint>,
    /// Le point d'arrêt où l'on vient de s'arrêter : l'instruction qui s'y trouve s'exécute au prochain `run`.
    resume_from: Option<u32>,
    /// Échantillons du profil d'exécution : `None` tant qu'il n'est pas lancé, sinon le nombre de fois où chaque
    /// adresse était celle de l'instruction en cours à l'arrêt d'une tranche.
    profile: Option<HashMap<u32, u64>>,
    /// Adresse de l'instruction fautive de la dernière faute.
    pub last_fault_pc: u32,
    /// Points d'arrêt et d'observation ignorés (pendant un rejeu de l'historique).
    debugging_suspended: bool,
}

/// Le fichier `.nwa` installé et ses données externes, tels que donnés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSource {
    pub nwa: Vec<u8>,
    pub data: Option<Vec<u8>>,
    /// Les octets placés en flash à part (`.nfd`), que l'application attend à des adresses fixes.
    pub flash: Vec<Segment>,
}

impl Emulator {
    /// Appareil du modèle demandé, avec le profil de firmware par défaut.
    pub fn new(model: Model) -> Result<Self, EmulatorError> {
        Self::with_profile(FirmwareProfile::default_for(model), KernelConfig::default())
    }

    pub fn with_profile(
        profile: FirmwareProfile,
        config: KernelConfig,
    ) -> Result<Self, EmulatorError> {
        let status = DeviceStatus::from_profile(&profile);
        let storage = Storage::new(profile.storage_address, profile.storage_size);
        let device = DeviceImage::new(profile)?;
        Ok(Self {
            device,
            cpu: Cpu::new(),
            kernel: Kernel::new(config, status),
            storage,
            app: None,
            source: None,
            flash: Vec::new(),
            debug: None,
            breakpoints: Vec::new(),
            resume_from: None,
            profile: None,
            last_fault_pc: 0,
            debugging_suspended: false,
        })
    }

    /// Lie un `.nwa` (avec ses données externes éventuelles), l'installe en flash et le prépare
    /// à démarrer.
    pub fn install_nwa(
        &mut self,
        nwa: &[u8],
        external_data: Option<&[u8]>,
    ) -> Result<(), EmulatorError> {
        let layout = self.device.app_layout();
        let app = link(nwa, external_data, &layout, &LinkOptions::default())?;
        let installed = app.flash_start..app.flash_start + app.installed_size();
        if let Some(segment) = self
            .flash
            .iter()
            .find(|s| s.address < installed.end && installed.start < s.end())
        {
            return Err(EmulatorError::Flash(format!(
                "l'application (0x{:08x}..0x{:08x}) recouvre le segment {}",
                installed.start,
                installed.end,
                describe(segment)
            )));
        }
        self.device.install(&app)?;
        self.kernel.flash_protected_end = self.device.next_app_address();
        self.app = Some(app);
        self.debug = None;
        self.source = Some(AppSource {
            nwa: nwa.to_vec(),
            data: external_data.map(<[u8]>::to_vec),
            flash: self.flash.clone(),
        });
        self.launch()
    }

    /// Place des octets dans la flash externe, comme on les flashe à part avec un outil USB : un gros
    /// programme ou des données que l'application attend à des adresses fixes. Les segments
    /// s'ajoutent à ceux déjà placés (un segment à la même adresse remplace l'ancien). Ils ne peuvent
    /// pas recouvrir le firmware ni les applications installées : à placer après l'application.
    pub fn install_flash(&mut self, segments: Vec<Segment>) -> Result<(), EmulatorError> {
        nfd::check_all(&segments).map_err(EmulatorError::Flash)?;
        let protected = self.device.next_app_address();
        for segment in &segments {
            if segment.address < protected {
                return Err(EmulatorError::Flash(format!(
                    "le segment {} recouvre le firmware ou une application installée (jusqu'à 0x{protected:08x})",
                    describe(segment)
                )));
            }
        }
        for segment in segments {
            self.flash.retain(|old| old.address != segment.address);
            self.device
                .memory
                .write_bytes(segment.address, &segment.data)
                .map_err(|e| EmulatorError::Flash(e.to_string()))?;
            self.flash.push(segment);
        }
        if let Some(source) = &mut self.source {
            source.flash = self.flash.clone();
        }
        self.kernel.flash_protected_end = protected;
        Ok(())
    }

    /// Le point d'arrêt d'où l'on repart (voir `resume_from`) : à sauvegarder avant d'exécuter du code pour le débogueur, et à rétablir après.
    pub fn resume_point(&self) -> Option<u32> {
        self.resume_from
    }

    pub fn set_resume_point(&mut self, point: Option<u32>) {
        self.resume_from = point;
    }

    /// Les informations de débogage du code source de l'application (DWARF), si elle en a.
    pub fn debug_info(&mut self) -> Option<Arc<DebugInfo>> {
        if self.debug.is_none() {
            let parsed = self.app.as_ref().and_then(|app| {
                let sections: Vec<(&str, &[u8])> = app
                    .debug
                    .iter()
                    .map(|s| (s.name.as_str(), s.data.as_slice()))
                    .collect();
                DebugInfo::from_sections(&sections)
                    .ok()
                    .flatten()
                    .map(|mut info| {
                        info.attach_symbols(
                            app.symbols.iter().map(|s| (s.address, s.name.as_str())),
                        );
                        Arc::new(info)
                    })
            });
            self.debug = Some(parsed);
        }
        self.debug.clone().flatten()
    }

    /// Les segments placés en flash par [`Emulator::install_flash`].
    pub fn flash_segments(&self) -> &[Segment] {
        &self.flash
    }

    /// Les fichiers de la calculatrice : consultables et modifiables à tout moment entre deux
    /// appels à [`Emulator::run`], même pendant que l'application est lancée.
    pub fn fs(&mut self) -> Fs<'_> {
        self.storage.with(&mut self.device.memory)
    }

    /// Les réglages de l'appareil (versions, nom, FCC, batterie…), modifiables à tout moment.
    pub fn settings(&mut self) -> Settings<'_> {
        Settings {
            device: &mut self.device,
            status: &mut self.kernel.status,
        }
    }

    /// Le `.nwa` (et ses données externes) installé en dernier, tel que donné.
    pub fn app_source(&self) -> Option<&AppSource> {
        self.source.as_ref()
    }

    /// Plage `[début, fin)` du tampon des fichiers en RAM.
    pub fn storage_range(&self) -> (u32, u32) {
        self.storage.range()
    }

    /// L'application installée en dernier.
    pub fn app(&self) -> Option<&LinkedApp> {
        self.app.as_ref()
    }

    /// (Re)démarre l'application : registres remis à zéro, pile du processus, retour de `main`
    /// vers le stub de fin, exécution à partir de `_start`.
    pub fn launch(&mut self) -> Result<(), EmulatorError> {
        if self.device.exam_mode {
            return Err(EmulatorError::ExamMode);
        }
        let app = self.app.as_ref().ok_or(EmulatorError::NoApp)?;
        let profile = &self.device.profile;
        let exit_stub = profile.active().userland_header + EXIT_STUB_OFFSET;
        self.device.memory.clear_access();
        let _ = crate::scb::reset(&mut self.device.memory);
        self.kernel.reset_waits();
        self.cpu = Cpu::new();
        self.cpu.regs.sp = profile.process_stack_start;
        self.cpu.regs.lr = exit_stub | 1;
        self.cpu.regs.pc = app.entry & !1;
        self.kernel
            .set_heap(app.heap.address, app.heap.address + app.heap.size);
        Ok(())
    }

    /// Temps de simulation écoulé depuis le début, en microsecondes : celui que voit l'application
    /// (`timing_millis`) et qui sert à ses attentes. Il avance quand l'hôte fait passer du temps
    /// (voir [`Kernel::advance_time`] et `ClockMode`), y compris pendant que l'application dort, et
    /// reste figé tant que l'hôte n'en fait pas passer (simulation en pause).
    pub fn time_us(&self) -> u64 {
        self.kernel.micros(self.cpu.cycles)
    }

    /// La fonction qui contient `address` (nom, début, décalage), d'après les symboles de l'application et du runtime.
    pub fn symbol_at(&self, address: u32) -> Option<(&str, u32, u32)> {
        let address = address & !1;
        let mut best: Option<(&str, u32)> = None;
        for symbol in &self.app.as_ref()?.symbols {
            if symbol.kind != SymbolKind::Func || symbol.origin == Origin::ExternalData {
                continue;
            }
            let start = symbol.address & !1;
            let inside = address >= start && (address < start + symbol.size || symbol.size == 0);
            if inside && best.is_none_or(|(_, s)| start >= s) {
                best = Some((symbol.name.as_str(), start));
            }
        }
        best.map(|(name, start)| (name, start, address - start))
    }

    /// Ignore (ou reprend) les points d'arrêt, les points d'observation et le profil : un rejeu de l'historique ne doit ni
    /// s'arrêter dessus ni les compter.
    pub fn suspend_debugging(&mut self, suspended: bool) {
        self.debugging_suspended = suspended;
        self.device.memory.suspend_watches(suspended);
        self.resume_from = None;
    }

    /// Indique que l'exécution est arrêtée sur le point d'arrêt de `pc` : la reprise exécute cette instruction au lieu de
    /// s'y arrêter de nouveau.
    pub fn stopped_at_breakpoint(&mut self) {
        self.resume_from = Some(self.cpu.regs.pc);
    }

    /// Pose un point d'arrêt : `run` s'interrompt avant d'exécuter l'instruction à cette adresse.
    pub fn add_breakpoint(&mut self, address: u32) {
        self.add_conditional_breakpoint(address, None);
    }

    /// Pose un point d'arrêt qui n'arrête l'exécution que si la condition est vérifiée ; en reposer un à la même adresse
    /// remplace le précédent (et sa condition).
    pub fn add_conditional_breakpoint(&mut self, address: u32, condition: Option<Condition>) {
        let address = address & !1;
        self.breakpoints.retain(|b| b.address != address);
        self.breakpoints.push(Breakpoint { address, condition });
    }

    /// Retire un point d'arrêt ; faux s'il n'existait pas.
    pub fn remove_breakpoint(&mut self, address: u32) -> bool {
        let before = self.breakpoints.len();
        self.breakpoints.retain(|b| b.address != address & !1);
        self.breakpoints.len() != before
    }

    /// Les points d'arrêt posés.
    pub fn breakpoints(&self) -> &[Breakpoint] {
        &self.breakpoints
    }

    /// Valeur d'un registre de cœur par son numéro (0 à 15).
    fn register(&self, number: u8) -> u32 {
        match number {
            0..=12 => self.cpu.regs.r[usize::from(number)],
            13 => self.cpu.regs.sp,
            14 => self.cpu.regs.lr,
            _ => self.cpu.regs.pc,
        }
    }

    /// Un point d'arrêt à `pc` s'applique-t-il ?
    fn breakpoint_hits(&self, pc: u32) -> bool {
        self.breakpoints.iter().any(|b| {
            b.address == pc
                && b.condition
                    .is_none_or(|c| c.compare.test(self.register(c.register), c.value))
        })
    }

    /// Lance (ou relance, en repartant de zéro) le profil d'exécution : l'adresse de l'instruction en cours est relevée toutes
    /// les quelques centaines d'instructions, ce qui ne coûte presque rien.
    pub fn start_profile(&mut self) {
        self.profile = Some(HashMap::new());
    }

    /// Arrête le profil et rend les relevés (adresse, nombre), ou rien s'il n'était pas lancé.
    pub fn stop_profile(&mut self) -> Option<Vec<(u32, u64)>> {
        self.profile.take().map(|m| m.into_iter().collect())
    }

    /// Les relevés du profil en cours (adresse, nombre), ou `None` s'il n'est pas lancé.
    pub fn profile_samples(&self) -> Option<Vec<(u32, u64)>> {
        self.profile
            .as_ref()
            .map(|m| m.iter().map(|(&a, &n)| (a, n)).collect())
    }

    /// Exécute une seule instruction, même si un point d'arrêt y est posé.
    pub fn step(&mut self) -> RunStop {
        self.resume_from = Some(self.cpu.regs.pc);
        self.run(1)
    }

    /// Exécute au plus `max_instructions` instructions de l'application. Une faute retient l'adresse de l'instruction fautive
    /// (`last_fault_pc`) et remplit les registres de faute du processeur simulé (voir [`crate::scb`]).
    pub fn run(&mut self, max_instructions: u64) -> RunStop {
        let stop = self.run_inner(max_instructions);
        if let RunStop::Fault(fault) = &stop {
            self.last_fault_pc = match fault {
                Fault::Bus(_) => self.cpu.fault_pc,
                Fault::Unimplemented { pc, .. } | Fault::UsageFault { pc, .. } => *pc,
                _ => self.cpu.regs.pc,
            };
            crate::scb::record(&mut self.device.memory, fault, self.last_fault_pc);
        }
        stop
    }

    fn run_inner(&mut self, max_instructions: u64) -> RunStop {
        if self.kernel.keyboard.exit_requested() {
            self.kernel.keyboard.clear_exit_request();
            return RunStop::HomePressed;
        }
        // Avec un point d'observation, on avance instruction par instruction : pour s'arrêter juste après
        // l'accès (point qui interrompt) et pour savoir quelle instruction l'a fait (adresse notée).
        let breaks = !self.debugging_suspended && !self.breakpoints.is_empty();
        let stepwise = self.device.memory.is_watching() || breaks;
        if !stepwise {
            self.resume_from = None;
        }
        let mut stop = Stop::Budget;
        let mut left = max_instructions;
        while left > 0 {
            // Avec le profil lancé, on exécute par tranches de longueur variable (un nombre premier plus ou moins
            // quelques unités : une longueur fixe retomberait toujours au même endroit d'une boucle).
            let chunk = if stepwise {
                1
            } else if self.profile.is_some() && !self.debugging_suspended {
                left.min(PROFILE_SLICE + (self.cpu.cycles >> 3) % 31)
            } else {
                left
            };
            if stepwise {
                let pc = self.cpu.regs.pc;
                // Le point d'arrêt où l'on s'est arrêté ne s'applique pas deux fois de suite à la même instruction.
                if self.resume_from.take() != Some(pc) && breaks && self.breakpoint_hits(pc) {
                    self.resume_from = Some(pc);
                    return RunStop::Breakpoint { pc };
                }
                self.device.memory.set_watch_pc(pc);
            }
            stop = self
                .cpu
                .run(&mut self.device.memory, &mut self.kernel, chunk);
            left -= chunk;
            if stop == Stop::Budget
                && !self.debugging_suspended
                && let Some(samples) = &mut self.profile
            {
                *samples.entry(self.cpu.regs.pc & !1).or_insert(0) += 1;
            }
            if let Some(hit) = self.device.memory.take_watch_stop() {
                return RunStop::Watchpoint(hit);
            }
            if stop != Stop::Budget {
                break;
            }
        }
        match stop {
            Stop::Budget => RunStop::Budget,
            Stop::Requested => match self.kernel.take_stop() {
                Some(KernelStop::Frame) => RunStop::Frame,
                Some(KernelStop::Sleep { ms }) => RunStop::Sleep { ms },
                Some(KernelStop::WaitEvent { remaining_ms }) => RunStop::WaitEvent { remaining_ms },
                Some(KernelStop::Exited(code)) => RunStop::Exited(code),
                Some(KernelStop::Reset) => RunStop::Reset,
                Some(KernelStop::HomePressed) => RunStop::HomePressed,
                Some(KernelStop::BusFault(f)) => {
                    // L'accès vient d'un appel système : l'instruction fautive est le `svc`, juste avant `pc`.
                    self.cpu.fault_pc = self.cpu.regs.pc.wrapping_sub(2);
                    RunStop::Fault(Fault::Bus(f))
                }
                Some(KernelStop::UnknownSvc(n)) => RunStop::Fault(Fault::UnknownSvc(n)),
                None => RunStop::Budget,
            },
            Stop::BusFault(f) => RunStop::Fault(Fault::Bus(f)),
            Stop::Unimplemented { pc, halfword } => {
                RunStop::Fault(Fault::Unimplemented { pc, halfword })
            }
            Stop::UsageFault { pc, kind } => RunStop::Fault(Fault::UsageFault { pc, kind }),
            Stop::Breakpoint { imm8 } => RunStop::Fault(Fault::Breakpoint { imm8 }),
            Stop::WaitForInterrupt => RunStop::Fault(Fault::WaitForInterrupt),
        }
    }
}

impl Emulator {
    /// Mesure l'utilisation du tas et de la pile d'après les accès du programme (au grain de 32
    /// octets). `None` tant qu'aucune application n'est installée.
    pub fn usage(&self) -> Option<Usage> {
        let app = self.app.as_ref()?;
        let profile = &self.device.profile;
        let heap = Span {
            start: app.heap.address,
            end: app.heap.address + app.heap.size,
        };
        let stack = Span {
            start: profile.apps_ram.1,
            end: profile.process_stack_start,
        };
        let memory = &self.device.memory;
        Some(Usage {
            heap,
            heap_break: self.kernel.program_break(),
            heap_high_water: memory
                .access_extent(heap.start, heap.end, access::WRITE)
                .map(|(_, high)| high),
            stack,
            stack_low_water: memory
                .access_extent(stack.start, stack.end, access::WRITE)
                .map(|(low, _)| low),
            sp: self.cpu.regs.sp,
        })
    }
}
