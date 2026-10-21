/// Registres du cœur (r0-r12, SP, LR, PC, xPSR) et registres de virgule flottante simple précision.
#[derive(Debug, Clone, Default)]
pub struct Registers {
    pub r: [u32; 13],
    pub sp: u32,
    pub lr: u32,
    pub pc: u32,
    pub xpsr: u32,
    /// Registres flottants s0-s31, stockés en bits bruts.
    pub s: [u32; 32],
    pub fpscr: u32,
}
