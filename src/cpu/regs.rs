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

