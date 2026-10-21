//! Exécution des instructions : le cycle « lire, décoder, exécuter » du processeur.

use super::alu::{
    ShiftKind, add_with_carry, shift_with_carry, sign_extend, signed_saturate, unsigned_saturate,
};
use super::decode::{decode16, decode32, is_32bit};
use super::instr::{
    Addr, DpOp, ExtKind, FlagMode, IndexMode, Instr, MemSize, ParallelOp, Reg, RevKind, Rhs,
    ShiftAmount, SysReg,
};
use super::{Bus, Cpu, Hooks, Stop, SvcAction, UsageFault};

/// Contexte de l'instruction en cours.
#[derive(Clone, Copy)]
struct Ctx {
    /// Adresse de l'instruction.
    addr: u32,
    /// Valeur lue quand l'instruction utilise PC comme opérande : adresse + 4.
    pc4: u32,
    /// L'instruction appartient à un bloc `IT` (les instructions de 16 bits ne modifient alors pas
    /// les indicateurs).
    in_it: bool,
}

impl Cpu {
    /// Condition ARM (4 bits) satisfaite avec les indicateurs actuels ?
    fn cond_passed(&self, cond: u8) -> bool {
        let r = &self.regs;
        let base = match cond >> 1 {
            0 => r.z(),
            1 => r.c(),
            2 => r.n(),
            3 => r.v(),
            4 => r.c() && !r.z(),
            5 => r.n() == r.v(),
            6 => !r.z() && r.n() == r.v(),
            _ => return true,
        };
        base != (cond & 1 != 0)
    }

    fn read(&self, ctx: Ctx, r: Reg) -> u32 {
        match r {
            15 => ctx.pc4,
            13 => self.regs.sp,
            14 => self.regs.lr,
            n => self.regs.r[n as usize],
        }
    }

    /// Écrit un registre autre que PC (les deux bits de poids faible de SP sont toujours nuls).
    fn write(&mut self, r: Reg, value: u32) {
        match r {
            13 => self.regs.sp = value & !3,
            14 => self.regs.lr = value,
            15 => unreachable!("PC s'écrit par un branchement"),
            n => self.regs.r[n as usize] = value,
        }
    }

    /// Écrit le résultat d'une opération : un branchement si la destination est PC.
    fn write_result(&mut self, rd: Reg, value: u32) {
        if rd == 15 {
            self.regs.pc = value & !1;
        } else {
            self.write(rd, value);
        }
    }

    /// Branchement avec échange de mode : le bit 0 doit être à 1 (Thumb) ; une valeur de type
    /// `EXC_RETURN` en mode « handler » est un retour d'exception.
    fn bx_write_pc<B: Bus>(&mut self, ctx: Ctx, bus: &mut B, target: u32) -> Result<(), Stop> {
        if self.handler_mode && target >> 28 == 0xf {
            return self.exception_return(bus, target);
        }
        if target & 1 == 0 {
            return Err(Stop::UsageFault {
                pc: ctx.addr,
                kind: UsageFault::InvalidState,
            });
        }
        self.regs.pc = target & !1;
        Ok(())
    }

    fn sets_flags(flags: FlagMode, ctx: Ctx) -> bool {
        match flags {
            FlagMode::Always => true,
            FlagMode::Never => false,
            FlagMode::OutsideIt => !ctx.in_it,
        }
    }

    /// Exécute une instruction à l'adresse `regs.pc`.
    pub fn step<B: Bus, H: Hooks>(&mut self, bus: &mut B, hooks: &mut H) -> Result<(), Stop> {
        let addr = self.regs.pc;
        let hw1 = bus.fetch16(addr).map_err(Stop::BusFault)?;
        let (decoded, size) = if is_32bit(hw1) {
            let hw2 = bus.fetch16(addr.wrapping_add(2)).map_err(Stop::BusFault)?;
            (decode32(hw1, hw2), 4)
        } else {
            (decode16(hw1), 2)
        };
        let instr = decoded.ok_or(Stop::Unimplemented {
            pc: addr,
            halfword: hw1,
        })?;
        let ctx = Ctx {
            addr,
            pc4: addr.wrapping_add(4),
            in_it: self.regs.it_state() != 0,
        };
        self.regs.pc = addr.wrapping_add(size);
        self.cycles += 1;

        let is_it = matches!(instr, Instr::It { .. });
        let skip = ctx.in_it && !is_it && !self.cond_passed(self.regs.it_state() >> 4);
        let outcome = if skip {
            Ok(())
        } else {
            self.execute(instr, ctx, bus, hooks)
        };
        if ctx.in_it && !is_it {
            self.regs.advance_it();
        }
        outcome
    }

    /// Exécute des instructions jusqu'à épuisement de `max_instructions` ou arrêt.
    pub fn run<B: Bus, H: Hooks>(
        &mut self,
        bus: &mut B,
        hooks: &mut H,
        max_instructions: u64,
    ) -> Stop {
        for _ in 0..max_instructions {
            if let Err(stop) = self.step(bus, hooks) {
                return stop;
            }
        }
        Stop::Budget
    }

    fn execute<B: Bus, H: Hooks>(
        &mut self,
        instr: Instr,
        ctx: Ctx,
        bus: &mut B,
        hooks: &mut H,
    ) -> Result<(), Stop> {
        match instr {
            Instr::Dp {
                op,
                rd,
                rn,
                rhs,
                flags,
            } => self.exec_dp(ctx, op, rd, rn, rhs, flags)?,
            Instr::Shift {
                kind,
                rd,
                value,
                amount,
                flags,
            } => {
                let v = self.read(ctx, value);
                let n = match amount {
                    ShiftAmount::Imm(n) => n,
                    ShiftAmount::Reg(r) => self.read(ctx, r) & 0xff,
                };
                let (result, carry) = shift_with_carry(v, kind, n, self.regs.c());
                if Self::sets_flags(flags, ctx) {
                    self.regs.set_nz(result);
                    self.regs.set_c(carry);
                }
                self.write_result(rd, result);
            }
            Instr::Movw { rd, imm } => self.write(rd, u32::from(imm)),
            Instr::Movt { rd, imm } => {
                let low = self.read(ctx, rd) & 0xffff;
                self.write(rd, (u32::from(imm) << 16) | low);
            }
            Instr::AddSubImm12 { rd, rn, imm, sub } => {
                let base = self.read(ctx, rn);
                self.write(
                    rd,
                    if sub {
                        base.wrapping_sub(imm)
                    } else {
                        base.wrapping_add(imm)
                    },
                );
            }
            Instr::Adr { rd, offset } => {
                self.write(rd, (ctx.pc4 & !3).wrapping_add(offset as u32));
            }
            Instr::Mul { rd, rn, rm, flags } => {
                let result = self.read(ctx, rn).wrapping_mul(self.read(ctx, rm));
                if Self::sets_flags(flags, ctx) {
                    self.regs.set_nz(result);
                }
                self.write(rd, result);
            }
            Instr::Mla { rd, rn, rm, ra } => {
                let product = self.read(ctx, rn).wrapping_mul(self.read(ctx, rm));
                self.write(rd, product.wrapping_add(self.read(ctx, ra)));
            }
            Instr::Mls { rd, rn, rm, ra } => {
                let product = self.read(ctx, rn).wrapping_mul(self.read(ctx, rm));
                self.write(rd, self.read(ctx, ra).wrapping_sub(product));
            }
