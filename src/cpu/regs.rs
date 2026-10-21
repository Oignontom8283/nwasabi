/// Registres de cœur (r0-r12, SP, LR, PC, xPSR) et registres de virgule flottante simple précision.
#[derive(Debug, Clone, Default)]
pub struct Registers {
    pub r: [u32; 13],
    pub sp: u32,
    pub lr: u32,
    /// Adresse de la prochaine instruction à exécuter.
    pub pc: u32,
    /// Registre d'état : indicateurs `N Z C V Q` (bits 31 à 27), bit Thumb (24), état du bloc `IT`
    /// (bits 26-25 et 15-10), indicateurs `GE` (bits 19 à 16), numéro d'exception (bits 8 à 0).
    pub xpsr: u32,
    /// Registres flottants s0-s31, stockés en bits bruts.
    pub s: [u32; 32],
    pub fpscr: u32,
}

const N: u32 = 1 << 31;
const Z: u32 = 1 << 30;
const C: u32 = 1 << 29;
const V: u32 = 1 << 28;
const Q: u32 = 1 << 27;
/// Bit « Thumb » de xPSR : toujours à 1 sur un Cortex-M.
pub const XPSR_T: u32 = 1 << 24;

impl Registers {
    pub fn n(&self) -> bool {
        self.xpsr & N != 0
    }

    pub fn z(&self) -> bool {
        self.xpsr & Z != 0
    }

    pub fn c(&self) -> bool {
        self.xpsr & C != 0
    }

    pub fn v(&self) -> bool {
        self.xpsr & V != 0
    }

    pub fn q(&self) -> bool {
        self.xpsr & Q != 0
    }

