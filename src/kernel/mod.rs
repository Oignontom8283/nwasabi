//! Le noyau simulé : ce qui répond à la place d'Epsilon quand l'application exécute `svc`.
//!
//! Aucun code d'Epsilon n'est utilisé : on reproduit seulement le comportement observable aux
//! frontières (valeurs rendues, écran, clavier, temps).
//!
//! Le noyau implémente [`Hooks`](crate::cpu::Hooks) : le processeur lui donne chaque `svc`. Quand
//! l'appel doit rendre la main à l'hôte (image prête, attente, fin de l'application…), le noyau
//! demande l'arrêt et l'hôte lit la raison avec [`Kernel::take_stop`].

pub mod display;
pub mod event_text;
pub mod font;
pub mod keyboard;
mod svc;
#[cfg(test)]
mod tests;

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

/// D'où vient le temps que voit l'application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockMode {
    /// Temps virtuel et déterministe : il avance avec les instructions exécutées (`cycles_per_us`) et
    /// par les attentes de l'application (`msleep`, image). C'est le mode des tests et de la ligne de
    /// commande : même exécution, même résultat, sans jamais attendre.
    #[default]
    Virtual,
    /// Temps de l'hôte : le temps n'avance que par [`Kernel::advance_time`], que l'hôte appelle avec le
    /// temps réellement écoulé. Les attentes de l'application (`msleep`, événements) durent donc le
    /// temps demandé, et si l'hôte met la simulation en pause sans avancer l'horloge, l'application
    /// ne voit pas le temps passer.
    Host,
}

/// Réglages du noyau simulé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelConfig {
    /// Origine du temps (voir [`ClockMode`]).
    pub clock: ClockMode,
    /// Cycles du processeur par microseconde : relie les instructions exécutées au temps virtuel.
    /// 0 : la fréquence du modèle (192 MHz sur N0110 et N0115, 540 MHz sur N0120).
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
            clock: ClockMode::Virtual,
            cycles_per_us: 0,
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
    /// Fréquence du processeur en MHz (relie les instructions au temps virtuel).
    pub cpu_mhz: u32,
}

/// Valeur de `COMPILATION_FLAGS` relevée dans le noyau d'Epsilon 26.4.0 des trois modèles.
const COMPILATION_FLAGS: u16 = 8;

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
            compilation_flags: COMPILATION_FLAGS,
            bootloader_crc: 0,
            last_reset_type: 0,
            cpu_mhz: profile.cpu_mhz,
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
    /// Nombre d'appels de chaque appel système, par numéro (statistiques).
    pub svc_counts: Vec<u32>,
    extra_us: u64,
    /// Fin de l'attente en cours (`msleep`, `usleep`) avec l'horloge de l'hôte.
    sleep_until_us: Option<u64>,
    random: u32,
    heap: (u32, u32),
    brk: u32,
    stop: Option<KernelStop>,
    wait: Option<EventWait>,
}

/// Taille maximale conservée de la sortie standard.
const MAX_STDOUT: usize = 1 << 20;

impl Kernel {
    pub fn new(mut config: KernelConfig, status: DeviceStatus) -> Self {
        if config.cycles_per_us == 0 {
            config.cycles_per_us = status.cpu_mhz;
        }
        Self {
            display: Display::new(),
            keyboard: Keyboard::new(),
            stdout: Vec::new(),
            frames: 0,
            unknown_svcs: 0,
            svc_counts: vec![0; 256],
            extra_us: 0,
            sleep_until_us: None,
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

    /// Temps écoulé depuis le démarrage, en microsecondes, pour `cycles` cycles de processeur déjà
    /// exécutés. Avec l'horloge de l'hôte, les instructions n'y comptent pour rien.
    pub fn micros(&self, cycles: u64) -> u64 {
        match self.config.clock {
            ClockMode::Virtual => {
                cycles / u64::from(self.config.cycles_per_us.max(1)) + self.extra_us
            }
            ClockMode::Host => self.extra_us,
        }
    }

    /// Fait passer du temps sans exécuter d'instructions. Avec l'horloge de l'hôte, c'est la seule
    /// façon de faire avancer le temps : l'hôte y met le temps réellement écoulé, et rien pendant une
    /// pause.
    pub fn advance_time(&mut self, us: u64) {
        self.extra_us = self.extra_us.saturating_add(us);
    }

    /// Oublie les attentes en cours (événement, `msleep`) : à appeler quand l'application repart de zéro.
    pub fn reset_waits(&mut self) {
        self.wait = None;
        self.sleep_until_us = None;
        self.stop = None;
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
