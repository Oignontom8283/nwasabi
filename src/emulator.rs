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
use crate::profile::{FirmwareProfile, Model};

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
    Fault(Fault),
}

/// Ce qui peut échouer en préparant l'application.
#[derive(Debug)]
pub enum EmulatorError {
    Link(LinkError),
    Device(DeviceError),
    /// Aucune application n'est installée.
    NoApp,
}

impl fmt::Display for EmulatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Link(e) => write!(f, "édition de liens : {e}"),
            Self::Device(e) => write!(f, "{e}"),
            Self::NoApp => write!(f, "aucune application installée"),
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
    app: Option<LinkedApp>,
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
        let device = DeviceImage::new(profile)?;
        Ok(Self {
            device,
            cpu: Cpu::new(),
            kernel: Kernel::new(config, status),
            app: None,
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
        self.launch()
    }

    /// L'application installée en dernier.
    pub fn app(&self) -> Option<&LinkedApp> {
        self.app.as_ref()
    }

    /// (Re)démarre l'application : registres remis à zéro, pile du processus, retour de `main`
    /// vers le stub de fin, exécution à partir de `_start`.
    pub fn launch(&mut self) -> Result<(), EmulatorError> {
        let app = self.app.as_ref().ok_or(EmulatorError::NoApp)?;
        let profile = &self.device.profile;
        let exit_stub = profile.active().userland_header + EXIT_STUB_OFFSET;
        self.cpu = Cpu::new();
        self.cpu.regs.sp = profile.process_stack_start;
        self.cpu.regs.lr = exit_stub | 1;
        self.cpu.regs.pc = app.entry & !1;
        self.kernel
            .set_heap(app.heap.address, app.heap.address + app.heap.size);
        Ok(())
    }

    /// Exécute au plus `max_instructions` instructions de l'application.
    pub fn run(&mut self, max_instructions: u64) -> RunStop {
        if self.kernel.keyboard.exit_requested() {
            self.kernel.keyboard.clear_exit_request();
            return RunStop::HomePressed;
        }
        let stop = self
            .cpu
            .run(&mut self.device.memory, &mut self.kernel, max_instructions);
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
