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

    fn set(&mut self, mask: u32, on: bool) {
        if on {
            self.xpsr |= mask;
        } else {
            self.xpsr &= !mask;
        }
    }

    pub fn set_n(&mut self, on: bool) {
        self.set(N, on);
    }

    pub fn set_z(&mut self, on: bool) {
        self.set(Z, on);
    }

    pub fn set_c(&mut self, on: bool) {
        self.set(C, on);
    }

    pub fn set_v(&mut self, on: bool) {
        self.set(V, on);
    }

    /// Indicateur de saturation « collant » : une fois levé il reste jusqu'à ce que le programme l'efface.
    pub fn set_q(&mut self) {
        self.xpsr |= Q;
    }

    /// Met à jour N et Z d'après `result`.
    pub fn set_nz(&mut self, result: u32) {
        self.set_n(result >> 31 != 0);
        self.set_z(result == 0);
    }

    /// Les quatre indicateurs `GE` (un bit par octet du résultat des opérations parallèles).
    pub fn ge(&self) -> u8 {
        ((self.xpsr >> 16) & 0xf) as u8
    }

    pub fn set_ge(&mut self, ge: u8) {
        self.xpsr = (self.xpsr & !(0xf << 16)) | (u32::from(ge & 0xf) << 16);
    }

    /// État du bloc `IT` en cours : 8 bits, `0` hors bloc (condition de base dans les 4 bits hauts).
    pub fn it_state(&self) -> u8 {
        (((self.xpsr >> 10) & 0x3f) << 2 | ((self.xpsr >> 25) & 3)) as u8
    }

    pub fn set_it_state(&mut self, it: u8) {
        let it = u32::from(it);
        self.xpsr = (self.xpsr & !(0x3f << 10) & !(3 << 25)) | ((it >> 2) << 10) | ((it & 3) << 25);
    }

    /// Fait avancer le bloc `IT` après une instruction qui y appartenait.
    pub fn advance_it(&mut self) {
        let it = self.it_state();
        let next = if it & 7 == 0 {
            0
        } else {
            (it & 0xe0) | ((it << 1) & 0x1f)
        };
        self.set_it_state(next);
    }

    /// Numéro d'exception en cours (0 : mode thread).
    pub fn ipsr(&self) -> u32 {
        self.xpsr & 0x1ff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_are_independent_bits_of_xpsr() {
        let mut r = Registers::default();
        r.set_n(true);
        r.set_c(true);
        assert_eq!(r.xpsr, 0xa000_0000);
        r.set_nz(0);
        assert_eq!((r.n(), r.z(), r.c(), r.v()), (false, true, true, false));
        r.set_q();
        assert!(r.q());
    }

    #[test]
    fn it_state_round_trips_through_its_scattered_bits() {
        let mut r = Registers::default();
        for it in 0..=255u8 {
            r.set_it_state(it);
            assert_eq!(r.it_state(), it);
        }
        // Les autres bits de xPSR ne bougent pas.
        r.xpsr = 0xf00f_0000;
        r.set_it_state(0xa5);
        assert_eq!(r.xpsr & 0xf00f_0000, 0xf00f_0000);
        assert_eq!(r.it_state(), 0xa5);
    }

    #[test]
    fn it_block_advances_and_ends() {
        let mut r = Registers::default();
        // `itte eq` : firstcond 0, mask 0b0110 -> état 0b0000_0110.
        r.set_it_state(0b0000_0110);
        r.advance_it();
        assert_eq!(r.it_state(), 0b0000_1100);
        r.advance_it();
        assert_eq!(r.it_state(), 0b0001_1000);
        // Quand les 3 bits de poids faible sont nuls, le bloc est terminé.
        r.set_it_state(0b0000_1000);
        r.advance_it();
        assert_eq!(r.it_state(), 0);
    }

    #[test]
    fn ge_flags_occupy_bits_19_to_16() {
        let mut r = Registers::default();
        r.set_ge(0b1010);
        assert_eq!(r.xpsr, 0b1010 << 16);
        assert_eq!(r.ge(), 0b1010);
    }
}
