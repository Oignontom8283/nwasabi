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
            Instr::MulLong {
                signed,
                accumulate,
                rdlo,
                rdhi,
                rn,
                rm,
            } => {
                let (a, b) = (self.read(ctx, rn), self.read(ctx, rm));
                let product: u64 = if signed {
                    (i64::from(a as i32) * i64::from(b as i32)) as u64
                } else {
                    u64::from(a) * u64::from(b)
                };
                let acc = if accumulate {
                    (u64::from(self.read(ctx, rdhi)) << 32) | u64::from(self.read(ctx, rdlo))
                } else {
                    0
                };
                let result = product.wrapping_add(acc);
                self.write(rdlo, result as u32);
                self.write(rdhi, (result >> 32) as u32);
            }
            Instr::Umaal { rdlo, rdhi, rn, rm } => {
                let product = u64::from(self.read(ctx, rn)) * u64::from(self.read(ctx, rm));
                let result =
                    product + u64::from(self.read(ctx, rdlo)) + u64::from(self.read(ctx, rdhi));
                self.write(rdlo, result as u32);
                self.write(rdhi, (result >> 32) as u32);
            }
            Instr::Div { signed, rd, rn, rm } => {
                let (a, b) = (self.read(ctx, rn), self.read(ctx, rm));
                // La division par zéro donne 0 (le piège n'est pas activé) ; MIN / -1 = MIN.
                let result = if b == 0 {
                    0
                } else if signed {
                    (a as i32).wrapping_div(b as i32) as u32
                } else {
                    a / b
                };
                self.write(rd, result);
            }
            Instr::Smul16 {
                rd,
                rn,
                rm,
                ra,
                n_high,
                m_high,
            } => {
                let half =
                    |v: u32, high: bool| i32::from((if high { v >> 16 } else { v }) as u16 as i16);
                let product = half(self.read(ctx, rn), n_high) * half(self.read(ctx, rm), m_high);
                let result = match ra {
                    None => product,
                    Some(ra) => {
                        let sum = i64::from(product) + i64::from(self.read(ctx, ra) as i32);
                        if sum != i64::from(sum as i32) {
                            self.regs.set_q();
                        }
                        sum as i32
                    }
                };
                self.write(rd, result as u32);
            }
            Instr::Smmul {
                rd,
                rn,
                rm,
                ra,
                round,
            } => {
                let product =
                    i64::from(self.read(ctx, rn) as i32) * i64::from(self.read(ctx, rm) as i32);
                let acc = ra.map_or(0, |ra| i64::from(self.read(ctx, ra) as i32) << 32);
                let total =
                    acc.wrapping_add(product)
                        .wrapping_add(if round { 0x8000_0000 } else { 0 });
                self.write(rd, (total >> 32) as u32);
            }
            Instr::Bfi { rd, rn, lsb, width } => {
                let source = rn.map_or(0, |rn| self.read(ctx, rn));
                let mask = field_mask(width) << lsb;
                let old = self.read(ctx, rd);
                self.write(rd, (old & !mask) | ((source << lsb) & mask));
            }
            Instr::Bfx {
                signed,
                rd,
                rn,
                lsb,
                width,
            } => {
                let field = (self.read(ctx, rn) >> lsb) & field_mask(width);
                let value = if signed {
                    sign_extend(field, u32::from(width)) as u32
                } else {
                    field
                };
                self.write(rd, value);
            }
            Instr::Sat {
                signed,
                rd,
                rn,
                bits,
                kind,
                amount,
            } => {
                let (operand, _) =
                    shift_with_carry(self.read(ctx, rn), kind, amount, self.regs.c());
                let operand = i64::from(operand as i32);
                let (result, saturated) = if signed {
                    let (r, s) = signed_saturate(operand, u32::from(bits));
                    (r as u32, s)
                } else {
                    unsigned_saturate(operand, u32::from(bits))
                };
                if saturated {
                    self.regs.set_q();
                }
                self.write(rd, result);
            }
            Instr::Clz { rd, rm } => self.write(rd, self.read(ctx, rm).leading_zeros()),
            Instr::Rbit { rd, rm } => self.write(rd, self.read(ctx, rm).reverse_bits()),
            Instr::Rev { kind, rd, rm } => {
                let v = self.read(ctx, rm);
                let result = match kind {
                    RevKind::Rev => v.swap_bytes(),
                    RevKind::Rev16 => ((v & 0xff00_ff00) >> 8) | ((v & 0x00ff_00ff) << 8),
                    RevKind::Revsh => sign_extend(u32::from((v as u16).swap_bytes()), 16) as u32,
                };
                self.write(rd, result);
            }
