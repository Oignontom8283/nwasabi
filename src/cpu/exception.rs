//! Entrée et retour d'exception (pile de contexte, `EXC_RETURN`).
//!
//! Utilisé quand un appel système `svc` n'est pas traité par l'hôte (`SvcAction::Raise`) : le
//! processeur empile son contexte, passe en mode « handler » et saute au vecteur d'exception.
//! C'est le mécanisme que le futur mode « firmware complet » utilisera.

use super::{Bus, Cpu, Stop, UsageFault};

/// Numéro d'exception de `SVCall`.
pub const EXC_SVCALL: u32 = 11;

/// Taille du contexte empilé : 8 mots entiers, plus 18 mots si le contexte flottant est actif.
const FRAME_INTEGER: u32 = 0x20;
const FRAME_FLOAT: u32 = 0x48;
const CONTROL_SPSEL: u32 = 1 << 1;
const CONTROL_FPCA: u32 = 1 << 2;
/// Bit de xPSR qui mémorise qu'un mot de remplissage a aligné la pile sur 8 octets.
const XPSR_ALIGN: u32 = 1 << 9;

impl Cpu {
    /// Empile le contexte et saute au gestionnaire de l'exception `exception`. L'instruction à
    /// laquelle revenir est `regs.pc` (la suivante).
    pub fn take_exception<B: Bus>(&mut self, bus: &mut B, exception: u32) -> Result<(), Stop> {
        // Une exception efface le moniteur d'exclusivité local.
        self.exclusive = None;
        let float = self.control & CONTROL_FPCA != 0;
        let size = FRAME_INTEGER + if float { FRAME_FLOAT } else { 0 };
        let aligned_by_padding = self.regs.sp & 4 != 0;
        let frame = self.regs.sp.wrapping_sub(size) & !(if aligned_by_padding { 4 } else { 0 });

        let mut words = [0u32; 8];
        words[..4].copy_from_slice(&self.regs.r[..4]);
        words[4] = self.regs.r[12];
        words[5] = self.regs.lr;
        words[6] = self.regs.pc;
        words[7] = (self.regs.xpsr & !XPSR_ALIGN) | if aligned_by_padding { XPSR_ALIGN } else { 0 };
        for (i, w) in words.iter().enumerate() {
            bus.write32(frame + 4 * i as u32, *w)
                .map_err(Stop::BusFault)?;
        }
        if float {
            for i in 0..16 {
                bus.write32(frame + 0x20 + 4 * i, self.regs.s[i as usize])
                    .map_err(Stop::BusFault)?;
            }
            bus.write32(frame + 0x60, self.regs.fpscr)
                .map_err(Stop::BusFault)?;
        }

        // Valeur de retour d'exception : mode et pile d'où l'on vient.
        let mut exc_return = if self.handler_mode {
            0xffff_fff1
        } else if self.control & CONTROL_SPSEL != 0 {
            0xffff_fffd
        } else {
            0xffff_fff9
        };
        if float {
            exc_return &= !0x10;
        }

        // Le gestionnaire s'exécute sur la pile principale.
        if !self.handler_mode && self.control & CONTROL_SPSEL != 0 {
            self.psp = frame;
            self.regs.sp = self.msp;
        } else {
            self.regs.sp = frame;
        }
        self.control &= !(CONTROL_SPSEL | CONTROL_FPCA);
        self.handler_mode = true;
        self.regs.lr = exc_return;
        self.regs.set_it_state(0);
        self.regs.xpsr = (self.regs.xpsr & !0x1ff) | exception;
        let vector = bus
            .read32(self.vtor.wrapping_add(4 * exception))
            .map_err(Stop::BusFault)?;
        self.regs.pc = vector & !1;
        Ok(())
    }

    /// Retour d'exception : dépile le contexte désigné par la valeur `EXC_RETURN`.
    pub(super) fn exception_return<B: Bus>(
        &mut self,
        bus: &mut B,
        exc_return: u32,
    ) -> Result<(), Stop> {
        self.exclusive = None;
        let (to_handler, use_psp) = match exc_return & 0xf {
            0x1 => (true, false),
            0x9 => (false, false),
            0xd => (false, true),
            _ => {
                return Err(Stop::UsageFault {
                    pc: self.regs.pc,
                    kind: UsageFault::InvalidState,
                });
            }
        };
        let float = exc_return & 0x10 == 0;
        let frame = if use_psp { self.psp } else { self.regs.sp };
        let word = |bus: &mut B, i: u32| {
            bus.read32(frame.wrapping_add(4 * i))
                .map_err(Stop::BusFault)
        };
        let mut words = [0u32; 8];
        for (i, w) in words.iter_mut().enumerate() {
            *w = word(bus, i as u32)?;
        }
        if float {
            for i in 0..16 {
                self.regs.s[i as usize] = word(bus, 8 + i)?;
            }
            self.regs.fpscr = word(bus, 24)?;
        }
        let size = FRAME_INTEGER + if float { FRAME_FLOAT } else { 0 };
        let new_sp = frame.wrapping_add(size) + if words[7] & XPSR_ALIGN != 0 { 4 } else { 0 };

        self.regs.r[..4].copy_from_slice(&words[..4]);
        self.regs.r[12] = words[4];
        self.regs.lr = words[5];
        self.regs.pc = words[6] & !1;
        self.regs.xpsr = words[7] & !XPSR_ALIGN;

        if use_psp {
            self.msp = self.regs.sp;
            self.psp = new_sp;
            self.regs.sp = new_sp;
            self.control |= CONTROL_SPSEL;
        } else {
            self.regs.sp = new_sp;
            self.control &= !CONTROL_SPSEL;
        }
        if float {
            self.control |= CONTROL_FPCA;
        }
        self.handler_mode = to_handler;
        Ok(())
    }
}
