//! Le noyau simulé : ce qui répond à la place d'Epsilon quand l'application exécute `svc`.
//!
//! Aucun code d'Epsilon n'est utilisé : on reproduit seulement le comportement observable aux
//! frontières (valeurs rendues, écran, clavier, temps).
//!
//! Le noyau implémente [`Hooks`](crate::cpu::Hooks) : le processeur lui donne chaque `svc`. Quand
//! l'appel doit rendre la main à l'hôte (image prête, attente, fin de l'application…), le noyau
//! demande l'arrêt et l'hôte lit la raison avec [`Kernel::take_stop`].

pub mod display;
pub mod font;
pub mod keyboard;
mod svc;

use crate::cpu::BusFault;
use crate::device::FCC_ID_OFFSET;
use crate::profile::FirmwareProfile;

pub use display::{Display, Rect};
pub use keyboard::{HomeBehavior, Key, Keyboard};

/// Durée d'une image de l'écran (60 Hz), en microsecondes.
pub const FRAME_US: u64 = 16_667;

/// Pourquoi le noyau a rendu la main à l'hôte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelStop {
    /// L'application attend la fin de l'image (`eadk_display_wait_for_vblank`) : l'écran est prêt.
    Frame,
    /// L'application dort ; le temps virtuel a déjà avancé de `ms` millisecondes.
    Sleep { ms: u32 },
    /// L'application attend un événement. `remaining_ms` est le délai restant (`None` : sans fin).
    /// L'appel sera refait quand l'hôte relance l'exécution.
    WaitEvent { remaining_ms: Option<u32> },
    /// L'application est terminée (retour de `main`) avec ce code.
    Exited(i32),
    /// Le noyau a repris la main : redémarrage demandé par l'application (`reset_core`).
    Reset,
    /// L'utilisateur a quitté l'application (touche `Home`).
    HomePressed,
    /// Un pointeur donné par l'application sort de la mémoire.
    BusFault(BusFault),
    /// Appel système inconnu (avec [`KernelConfig::strict_svc`]).
    UnknownSvc(u8),
}

/// Réglages du noyau simulé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelConfig {
    /// Cycles du processeur par microseconde : relie les instructions exécutées au temps virtuel
    /// (216 pour le processeur à 216 MHz du N0110).
    pub cycles_per_us: u32,
    /// Un appel système inconnu arrête l'application. Sinon il renvoie 0 et on continue.
    pub strict_svc: bool,
    /// `write` (sortie standard du C) dessine aussi le texte en haut de l'écran, comme l'appareil.
    pub stdout_to_screen: bool,
    /// Graine du générateur aléatoire (même graine, même suite : exécutions reproductibles).
    pub random_seed: u32,
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            cycles_per_us: 216,
            strict_svc: true,
            stdout_to_screen: true,
            random_seed: 0x1234_5678,
        }
    }
}

/// Ce que l'appareil répond sur lui-même (modifiable par l'hôte).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceStatus {
    pub brightness: u8,
    pub battery_level: u8,
    pub battery_voltage: f32,
    pub battery_charging: bool,
    pub usb_plugged: bool,
    pub clearance_level: u32,
    pub pcb_version: u32,
    pub serial_number: String,
    /// Adresse en mémoire de la chaîne de l'identifiant FCC.
    pub fcc_id_address: u32,
    pub compilation_flags: u16,
    pub bootloader_crc: u32,
    pub last_reset_type: u8,
}

impl DeviceStatus {
    /// Valeurs plausibles pour un appareil neuf, tirées du profil du firmware.
    pub fn from_profile(profile: &FirmwareProfile) -> Self {
        Self {
            brightness: 240,
            battery_level: 4,
            battery_voltage: 4.0,
            battery_charging: false,
            usb_plugged: false,
            clearance_level: 1,
            pcb_version: profile.pcb_version,
            serial_number: profile.serial_number.clone(),
            fcc_id_address: profile.active().userland_header + FCC_ID_OFFSET,
            compilation_flags: 0,
            bootloader_crc: 0,
            last_reset_type: 0,
        }
    }
}

/// Attente d'un événement en cours (l'appel système est refait à chaque reprise).
#[derive(Debug, Clone, Copy)]
struct EventWait {
    started_us: u64,
    /// Délai demandé par l'application (millisecondes), négatif : sans limite.
    timeout_ms: i32,
}

/// Le noyau simulé.
#[derive(Debug, Clone)]
pub struct Kernel {
    pub config: KernelConfig,
    pub display: Display,
    pub keyboard: Keyboard,
    pub status: DeviceStatus,
    /// Texte écrit par l'application sur la sortie standard.
    pub stdout: Vec<u8>,
    /// Nombre d'images terminées (`wait_for_vblank`).
    pub frames: u64,
    /// Appels système inconnus rencontrés quand `strict_svc` est faux.
    pub unknown_svcs: u32,
    extra_us: u64,
    random: u32,
    heap: (u32, u32),
    brk: u32,
    stop: Option<KernelStop>,
    wait: Option<EventWait>,
}

/// Taille maximale conservée de la sortie standard.
const MAX_STDOUT: usize = 1 << 20;

impl Kernel {
    pub fn new(config: KernelConfig, status: DeviceStatus) -> Self {
        Self {
            display: Display::new(),
            keyboard: Keyboard::new(),
            stdout: Vec::new(),
            frames: 0,
            unknown_svcs: 0,
            extra_us: 0,
            random: config.random_seed.max(1),
            heap: (0, 0),
            brk: 0,
            stop: None,
            wait: None,
            config,
            status,
        }
    }

    /// Définit le tas de l'application `[start, end)` et y place la limite du programme.
    pub fn set_heap(&mut self, start: u32, end: u32) {
        self.heap = (start, end);
        self.brk = start;
    }

    /// Limite actuelle du tas.
    pub fn program_break(&self) -> u32 {
        self.brk
    }

    /// Temps virtuel écoulé depuis le démarrage, en microsecondes, pour `cycles` cycles de
    /// processeur déjà exécutés.
    pub fn micros(&self, cycles: u64) -> u64 {
        cycles / u64::from(self.config.cycles_per_us.max(1)) + self.extra_us
    }

    /// Fait passer du temps sans exécuter d'instructions (attente d'un événement, hôte en
    /// temps réel…).
    pub fn advance_time(&mut self, us: u64) {
        self.extra_us = self.extra_us.saturating_add(us);
    }

    /// Prend la raison pour laquelle le noyau a rendu la main, s'il y en a une.
    pub fn take_stop(&mut self) -> Option<KernelStop> {
        self.stop.take()
    }

    fn random_next(&mut self) -> u32 {
        // xorshift32
        let mut x = self.random;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.random = x;
        x
    }

    fn write_stdout(&mut self, bytes: &[u8]) {
        let room = MAX_STDOUT.saturating_sub(self.stdout.len());
        self.stdout
            .extend_from_slice(&bytes[..bytes.len().min(room)]);
        if self.config.stdout_to_screen {
            self.display.draw_text(bytes, 0, 0, false, 0, 0xffff);
        }
    }
}
