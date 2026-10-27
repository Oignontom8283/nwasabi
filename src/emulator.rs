//! L'émulateur : un appareil, un processeur et le noyau simulé, réunis pour exécuter une
//! application `.nwa`.
//!
//! L'exécution est coopérative : [`Emulator::run`] avance l'application jusqu'à un point où
//! l'hôte doit intervenir (image prête, attente d'un événement, fin…) et dit pourquoi elle
//! s'est arrêtée. Le cœur ne dort jamais et ne lit aucune horloge : c'est à l'hôte de faire
//! passer le temps ([`Kernel::advance_time`]).

use std::fmt;

use crate::cpu::{BusFault, Cpu, Stop, UsageFault};
use crate::device::{DeviceError, DeviceImage, EXIT_STUB_OFFSET};
use crate::kernel::{DeviceStatus, Kernel, KernelConfig, KernelStop};
use crate::link::{LinkError, LinkOptions, LinkedApp, link};
use crate::memory::{WatchHit, access};
use crate::profile::{FirmwareProfile, Model};
use crate::settings::Settings;
use crate::storage::{Fs, Storage};

/// Une faute qui arrête l'application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    Bus(BusFault),
    /// Instruction que l'émulateur ne sait pas (encore) exécuter.
    Unimplemented {
        pc: u32,
        halfword: u16,
    },
    UsageFault {
        pc: u32,
        kind: UsageFault,
    },
    /// Instruction `bkpt`.
    Breakpoint {
        imm8: u8,
    },
    /// `wfi` / `wfe` : rien ne pourrait réveiller le processeur.
    WaitForInterrupt,
    /// Appel système inconnu du noyau simulé.
    UnknownSvc(u8),
}

/// Pourquoi [`Emulator::run`] a rendu la main.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStop {
    /// L'image est prête : l'écran peut être affiché.
    Frame,
    /// L'application dort (le temps virtuel a déjà avancé).
    Sleep {
        ms: u32,
    },
    /// L'application attend un événement du clavier ; l'hôte doit faire passer du temps ou
    /// appuyer sur une touche avant de relancer.
    WaitEvent {
        remaining_ms: Option<u32>,
    },
    /// `main` est revenue avec ce code.
    Exited(i32),
    /// L'application a demandé un redémarrage.
    Reset,
    /// L'utilisateur a quitté l'application avec `Home`.
    HomePressed,
    /// Le budget d'instructions est épuisé.
    Budget,
    /// Un point d'observation « avec arrêt » a été touché (voir `Memory::watch`).
    Watchpoint(WatchHit),
    Fault(Fault),
}

/// Ce qui peut échouer en préparant l'application.
#[derive(Debug)]
pub enum EmulatorError {
    Link(LinkError),
    Device(DeviceError),
    /// Aucune application n'est installée.
    NoApp,
    /// Le mode examen cache les applications externes.
    ExamMode,
}

impl fmt::Display for EmulatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Link(e) => write!(f, "édition de liens : {e}"),
            Self::Device(e) => write!(f, "{e}"),
            Self::NoApp => write!(f, "aucune application installée"),
            Self::ExamMode => write!(f, "mode examen : les applications externes sont cachées"),
        }
    }
}

impl std::error::Error for EmulatorError {}

impl From<LinkError> for EmulatorError {
    fn from(e: LinkError) -> Self {
        Self::Link(e)
    }
}

impl From<DeviceError> for EmulatorError {
    fn from(e: DeviceError) -> Self {
        Self::Device(e)
    }
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
}

/// Le fichier `.nwa` installé et ses données externes, tels que donnés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSource {
    pub nwa: Vec<u8>,
    pub data: Option<Vec<u8>>,
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
        self.device.install(&app)?;
        self.app = Some(app);
        self.source = Some(AppSource {
            nwa: nwa.to_vec(),
            data: external_data.map(<[u8]>::to_vec),
        });
        self.launch()
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

    /// Exécute au plus `max_instructions` instructions de l'application.
    pub fn run(&mut self, max_instructions: u64) -> RunStop {
        if self.kernel.keyboard.exit_requested() {
            self.kernel.keyboard.clear_exit_request();
            return RunStop::HomePressed;
        }
        // Avec un point d'observation, on avance instruction par instruction : pour s'arrêter juste après
        // l'accès (point qui interrompt) et pour savoir quelle instruction l'a fait (adresse notée).
        let stepwise = self.device.memory.is_watching();
        let mut stop = Stop::Budget;
        let mut left = max_instructions;
        while left > 0 {
            let chunk = if stepwise { 1 } else { left };
            if stepwise {
                self.device.memory.set_watch_pc(self.cpu.regs.pc);
            }
            stop = self
                .cpu
                .run(&mut self.device.memory, &mut self.kernel, chunk);
            left -= chunk;
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
                Some(KernelStop::BusFault(f)) => RunStop::Fault(Fault::Bus(f)),
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

/// Une plage d'adresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

/// Utilisation de la mémoire par l'application depuis son lancement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Plage réservée au tas, limite actuelle du programme (`brk`) et plus haute adresse écrite.
    pub heap: Span,
    pub heap_break: u32,
    pub heap_high_water: Option<u32>,
    /// Plage réservée à la pile (la pile descend depuis `end`) et adresse la plus basse écrite.
    pub stack: Span,
    pub stack_low_water: Option<u32>,
    /// Pile actuelle.
    pub sp: u32,
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
