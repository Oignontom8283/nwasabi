//! Désassembleur Thumb-2 : met en texte les instructions que le décodeur reconnaît (syntaxe unifiée d'ARM).
//!
//! Il lit la mémoire par une fonction qui rend des demi-mots, sans dépendre d'autre chose que du décodeur. Les blocs `it`
//! sont suivis le long d'une suite d'instructions : celles qu'ils rendent conditionnelles prennent leur suffixe (`addeq`).

use super::alu::ShiftKind;
use super::decode::{decode16, decode32, is_32bit};
use super::fpu::{FpArith, FpInstr, FpUnary};
use super::instr::{
    Addr, DpOp, ExtKind, FlagMode, IndexMode, Instr, MemSize, ParallelOp, Reg, RevKind, Rhs,
    ShiftAmount,
};

/// Une instruction désassemblée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub address: u32,
    /// Taille en octets (2 ou 4).
    pub size: u8,
    /// Les demi-mots de l'instruction (le second vaut 0 pour une instruction de 16 bits).
    pub halfwords: [u16; 2],
    pub mnemonic: String,
    pub operands: String,
    /// Adresse visée : cible d'un branchement, ou mot lu relativement au PC.
    pub target: Option<u32>,
}

const CONDITIONS: [&str; 15] = [
    "eq", "ne", "cs", "cc", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "",
];

fn reg(r: Reg) -> String {
    match r {
        13 => "sp".into(),
        14 => "lr".into(),
        15 => "pc".into(),
        n => format!("r{n}"),
    }
}

fn imm(value: u32) -> String {
    if value < 256 {
        format!("#{value}")
    } else {
        format!("#{value:#x}")
    }
}

fn simm(value: i32) -> String {
    if value < 0 {
        format!("#-{}", imm(value.unsigned_abs()).trim_start_matches('#'))
    } else {
        imm(value as u32)
    }
}

fn shift_name(kind: ShiftKind) -> &'static str {
    match kind {
        ShiftKind::Lsl => "lsl",
        ShiftKind::Lsr => "lsr",
        ShiftKind::Asr => "asr",
        ShiftKind::Ror => "ror",
        ShiftKind::Rrx => "rrx",
    }
}

fn reg_list(mask: u16) -> String {
    let names: Vec<String> = (0..16)
        .filter(|i| mask >> i & 1 != 0)
        .map(|i| reg(i as Reg))
        .collect();
    format!("{{{}}}", names.join(", "))
}

fn rhs(r: Rhs) -> String {
    match r {
        Rhs::Imm { value, .. } => imm(value),
        Rhs::Reg { rm, kind, amount } => match (kind, amount) {
            (ShiftKind::Lsl, 0) => reg(rm),
            (ShiftKind::Rrx, _) => format!("{}, rrx", reg(rm)),
            _ => format!("{}, {} {}", reg(rm), shift_name(kind), imm(amount)),
        },
    }
}

fn address(a: Addr) -> String {
    match a {
        Addr::Imm { rn, offset, mode } => match mode {
            IndexMode::Offset if offset == 0 => format!("[{}]", reg(rn)),
            IndexMode::Offset => format!("[{}, {}]", reg(rn), simm(offset)),
            IndexMode::PreIndex => format!("[{}, {}]!", reg(rn), simm(offset)),
            IndexMode::PostIndex => format!("[{}], {}", reg(rn), simm(offset)),
        },
        Addr::Reg { rn, rm, shift: 0 } => format!("[{}, {}]", reg(rn), reg(rm)),
        Addr::Reg { rn, rm, shift } => {
            format!("[{}, {}, lsl {}]", reg(rn), reg(rm), imm(u32::from(shift)))
        }
        Addr::Literal { offset } => format!("[pc, {}]", simm(offset)),
    }
}

fn indexed(rn: Reg, offset: i32, mode: IndexMode) -> String {
    address(Addr::Imm { rn, offset, mode })
}

fn sysreg(n: u8) -> String {
    match n {
        0 => "apsr".into(),
        1 => "iapsr".into(),
        2 => "eapsr".into(),
        3 => "xpsr".into(),
        5 => "ipsr".into(),
        6 => "epsr".into(),
        7 => "iepsr".into(),
        8 => "msp".into(),
        9 => "psp".into(),
        16 => "primask".into(),
        17 => "basepri".into(),
        18 => "basepri_max".into(),
        19 => "faultmask".into(),
        20 => "control".into(),
        other => format!("sysreg{other}"),
    }
}

fn s_reg(n: u8) -> String {
    format!("s{n}")
}

fn d_reg(n: u8) -> String {
    format!("d{}", n / 2)
}

/// Le texte (mnémonique, opérandes) et l'adresse visée éventuelle d'une instruction placée à `pc`.
/// `in_it` : l'instruction est dans un bloc `it` (les formes de 16 bits ne modifient alors pas les indicateurs).
fn render(instr: &Instr, pc: u32, size: u32, in_it: bool) -> (String, String, Option<u32>) {
    let pc4 = pc.wrapping_add(4);
    let _ = size;
    let flag = |f: FlagMode| match f {
        FlagMode::Always => "s",
        FlagMode::Never => "",
        FlagMode::OutsideIt => {
            if in_it {
                ""
            } else {
                "s"
            }
        }
    };
    let branch = |offset: i32| pc4.wrapping_add(offset as u32);
    let m = |text: &str, ops: String| (text.to_string(), ops, None);
    match *instr {
        Instr::Dp {
            op,
            rd,
            rn,
            rhs: r,
            flags,
        } => {
            let name = match op {
                DpOp::And => "and",
                DpOp::Eor => "eor",
                DpOp::Sub => "sub",
                DpOp::Rsb => "rsb",
                DpOp::Add => "add",
                DpOp::Adc => "adc",
                DpOp::Sbc => "sbc",
                DpOp::Orr => "orr",
                DpOp::Orn => "orn",
                DpOp::Bic => "bic",
                DpOp::Mov => "mov",
                DpOp::Mvn => "mvn",
                DpOp::Tst => "tst",
                DpOp::Teq => "teq",
                DpOp::Cmp => "cmp",
                DpOp::Cmn => "cmn",
            };
            match op {
                DpOp::Mov | DpOp::Mvn => m(
                    &format!("{name}{}", flag(flags)),
                    format!("{}, {}", reg(rd), rhs(r)),
                ),
                DpOp::Tst | DpOp::Teq | DpOp::Cmp | DpOp::Cmn => {
                    m(name, format!("{}, {}", reg(rn), rhs(r)))
                }
                _ => m(
                    &format!("{name}{}", flag(flags)),
                    format!("{}, {}, {}", reg(rd), reg(rn), rhs(r)),
                ),
            }
        }
        Instr::Shift {
            kind,
            rd,
            value,
            amount,
            flags,
        } => {
            let name = format!("{}{}", shift_name(kind), flag(flags));
            let ops = match (kind, amount) {
                (ShiftKind::Rrx, _) => format!("{}, {}", reg(rd), reg(value)),
                (_, ShiftAmount::Imm(n)) => format!("{}, {}, {}", reg(rd), reg(value), imm(n)),
                (_, ShiftAmount::Reg(rs)) => format!("{}, {}, {}", reg(rd), reg(value), reg(rs)),
            };
            m(&name, ops)
        }
        Instr::Movw { rd, imm: v } => m("movw", format!("{}, {}", reg(rd), imm(u32::from(v)))),
        Instr::Movt { rd, imm: v } => m("movt", format!("{}, {}", reg(rd), imm(u32::from(v)))),
        Instr::AddSubImm12 {
            rd,
            rn,
            imm: v,
            sub,
        } => m(
            if sub { "subw" } else { "addw" },
            format!("{}, {}, {}", reg(rd), reg(rn), imm(v)),
        ),
        Instr::Adr { rd, offset } => {
            let target = (pc4 & !3).wrapping_add(offset as u32);
            (
                "adr".into(),
                format!("{}, {target:#x}", reg(rd)),
                Some(target),
            )
        }
        Instr::Mul { rd, rn, rm, flags } => m(
            &format!("mul{}", flag(flags)),
            format!("{}, {}, {}", reg(rd), reg(rn), reg(rm)),
        ),
        Instr::Mla { rd, rn, rm, ra } => m(
            "mla",
            format!("{}, {}, {}, {}", reg(rd), reg(rn), reg(rm), reg(ra)),
        ),
        Instr::Mls { rd, rn, rm, ra } => m(
            "mls",
            format!("{}, {}, {}, {}", reg(rd), reg(rn), reg(rm), reg(ra)),
        ),
        Instr::MulLong {
            signed,
            accumulate,
            rdlo,
            rdhi,
            rn,
            rm,
        } => {
            let name = match (signed, accumulate) {
                (true, false) => "smull",
                (false, false) => "umull",
                (true, true) => "smlal",
                (false, true) => "umlal",
            };
            m(
                name,
                format!("{}, {}, {}, {}", reg(rdlo), reg(rdhi), reg(rn), reg(rm)),
            )
        }
        Instr::Umaal { rdlo, rdhi, rn, rm } => m(
            "umaal",
            format!("{}, {}, {}, {}", reg(rdlo), reg(rdhi), reg(rn), reg(rm)),
        ),
        Instr::Div { signed, rd, rn, rm } => m(
            if signed { "sdiv" } else { "udiv" },
            format!("{}, {}, {}", reg(rd), reg(rn), reg(rm)),
        ),
        Instr::Smul16 {
            rd,
            rn,
            rm,
            ra,
            n_high,
            m_high,
        } => {
            let xy = format!(
                "{}{}",
                if n_high { 't' } else { 'b' },
                if m_high { 't' } else { 'b' }
            );
            match ra {
                Some(ra) => m(
                    &format!("smla{xy}"),
                    format!("{}, {}, {}, {}", reg(rd), reg(rn), reg(rm), reg(ra)),
                ),
                None => m(
                    &format!("smul{xy}"),
                    format!("{}, {}, {}", reg(rd), reg(rn), reg(rm)),
                ),
            }
        }
        Instr::Smmul {
            rd,
            rn,
            rm,
            ra,
            round,
        } => {
            let r = if round { "r" } else { "" };
            match ra {
                Some(ra) => m(
                    &format!("smmla{r}"),
                    format!("{}, {}, {}, {}", reg(rd), reg(rn), reg(rm), reg(ra)),
                ),
                None => m(
                    &format!("smmul{r}"),
                    format!("{}, {}, {}", reg(rd), reg(rn), reg(rm)),
                ),
            }
        }
        Instr::Bfi { rd, rn, lsb, width } => match rn {
            Some(rn) => m(
                "bfi",
                format!(
                    "{}, {}, {}, {}",
                    reg(rd),
                    reg(rn),
                    imm(u32::from(lsb)),
                    imm(u32::from(width))
                ),
            ),
            None => m(
                "bfc",
                format!(
                    "{}, {}, {}",
                    reg(rd),
                    imm(u32::from(lsb)),
                    imm(u32::from(width))
                ),
            ),
        },
        Instr::Bfx {
            signed,
            rd,
            rn,
            lsb,
            width,
        } => m(
            if signed { "sbfx" } else { "ubfx" },
            format!(
                "{}, {}, {}, {}",
                reg(rd),
                reg(rn),
                imm(u32::from(lsb)),
                imm(u32::from(width))
            ),
        ),
        Instr::Sat {
            signed,
            rd,
            rn,
            bits,
            kind,
            amount,
        } => {
            let shifted = if amount == 0 {
                reg(rn)
            } else {
                format!("{}, {} {}", reg(rn), shift_name(kind), imm(amount))
            };
            m(
                if signed { "ssat" } else { "usat" },
                format!("{}, {}, {}", reg(rd), imm(u32::from(bits)), shifted),
            )
        }
        Instr::Clz { rd, rm } => m("clz", format!("{}, {}", reg(rd), reg(rm))),
        Instr::Rbit { rd, rm } => m("rbit", format!("{}, {}", reg(rd), reg(rm))),
        Instr::Rev { kind, rd, rm } => m(
            match kind {
                RevKind::Rev => "rev",
                RevKind::Rev16 => "rev16",
                RevKind::Revsh => "revsh",
            },
            format!("{}, {}", reg(rd), reg(rm)),
        ),
        Instr::Extend {
            kind,
            rd,
            rm,
            rotate,
            rn,
        } => {
            let name = match (kind, rn.is_some()) {
                (ExtKind::Sxtb, false) => "sxtb",
                (ExtKind::Sxth, false) => "sxth",
                (ExtKind::Uxtb, false) => "uxtb",
                (ExtKind::Uxth, false) => "uxth",
                (ExtKind::Sxtb16, false) => "sxtb16",
                (ExtKind::Uxtb16, false) => "uxtb16",
                (ExtKind::Sxtb, true) => "sxtab",
                (ExtKind::Sxth, true) => "sxtah",
                (ExtKind::Uxtb, true) => "uxtab",
                (ExtKind::Uxth, true) => "uxtah",
                (ExtKind::Sxtb16, true) => "sxtab16",
                (ExtKind::Uxtb16, true) => "uxtab16",
            };
            let mut ops = reg(rd);
            if let Some(rn) = rn {
                ops += &format!(", {}", reg(rn));
            }
            ops += &format!(", {}", reg(rm));
            if rotate != 0 {
                ops += &format!(", ror {}", imm(u32::from(rotate)));
            }
            m(name, ops)
        }
        Instr::Parallel { op, rd, rn, rm } => {
            let name = match op {
                ParallelOp::Uadd8 => "uadd8",
                ParallelOp::Uqadd8 => "uqadd8",
                ParallelOp::Uqsub8 => "uqsub8",
                ParallelOp::Uqadd16 => "uqadd16",
                ParallelOp::Uqsub16 => "uqsub16",
                ParallelOp::Sadd8 => "sadd8",
                ParallelOp::Ssub8 => "ssub8",
                ParallelOp::Usub8 => "usub8",
                ParallelOp::Uadd16 => "uadd16",
                ParallelOp::Usub16 => "usub16",
                ParallelOp::Sadd16 => "sadd16",
                ParallelOp::Ssub16 => "ssub16",
            };
            m(name, format!("{}, {}, {}", reg(rd), reg(rn), reg(rm)))
        }
        Instr::Sel { rd, rn, rm } => m("sel", format!("{}, {}, {}", reg(rd), reg(rn), reg(rm))),
        Instr::Pkh {
            top_bottom,
            rd,
            rn,
            rm,
            amount,
        } => {
            let shifted = if amount == 0 {
                reg(rm)
            } else {
                format!(
                    "{}, {} {}",
                    reg(rm),
                    if top_bottom { "asr" } else { "lsl" },
                    imm(amount)
                )
            };
            m(
                if top_bottom { "pkhtb" } else { "pkhbt" },
                format!("{}, {}, {}", reg(rd), reg(rn), shifted),
            )
        }
        Instr::B { offset } => {
            let t = branch(offset);
            ("b".into(), format!("{t:#x}"), Some(t))
        }
        Instr::BCond { cond, offset } => {
            let t = branch(offset);
            (
                format!(
                    "b{}",
                    CONDITIONS.get(usize::from(cond)).copied().unwrap_or("")
                ),
                format!("{t:#x}"),
                Some(t),
            )
        }
        Instr::Bl { offset } => {
            let t = branch(offset);
            ("bl".into(), format!("{t:#x}"), Some(t))
        }
        Instr::Bx { rm } => m("bx", reg(rm)),
        Instr::Blx { rm } => m("blx", reg(rm)),
        Instr::Cbz {
            rn,
            offset,
            nonzero,
        } => {
            let t = pc4.wrapping_add(offset);
            (
                if nonzero { "cbnz" } else { "cbz" }.into(),
                format!("{}, {t:#x}", reg(rn)),
                Some(t),
            )
        }
        Instr::TableBranch { half, rn, rm } => {
            if half {
                m("tbh", format!("[{}, {}, lsl #1]", reg(rn), reg(rm)))
            } else {
                m("tbb", format!("[{}, {}]", reg(rn), reg(rm)))
            }
        }
        Instr::Load {
            size,
            signed,
            rt,
            addr,
        } => {
            let name = match (size, signed) {
                (MemSize::Word, _) => "ldr",
                (MemSize::Half, false) => "ldrh",
                (MemSize::Half, true) => "ldrsh",
                (MemSize::Byte, false) => "ldrb",
                (MemSize::Byte, true) => "ldrsb",
            };
            let target = match addr {
                Addr::Literal { offset } => Some((pc4 & !3).wrapping_add(offset as u32)),
                _ => None,
            };
            (
                name.into(),
                format!("{}, {}", reg(rt), address(addr)),
                target,
            )
        }
        Instr::Store { size, rt, addr } => {
            let name = match size {
                MemSize::Word => "str",
                MemSize::Half => "strh",
                MemSize::Byte => "strb",
            };
            m(name, format!("{}, {}", reg(rt), address(addr)))
        }
        Instr::Ldrd {
            rt,
            rt2,
            rn,
            offset,
            mode,
        } => m(
            "ldrd",
            format!("{}, {}, {}", reg(rt), reg(rt2), indexed(rn, offset, mode)),
        ),
        Instr::Strd {
            rt,
            rt2,
            rn,
            offset,
            mode,
        } => m(
            "strd",
            format!("{}, {}, {}", reg(rt), reg(rt2), indexed(rn, offset, mode)),
        ),
        Instr::Ldm {
            rn,
            regs,
            writeback,
            decrement_before,
        } => {
            if rn == 13 && writeback && !decrement_before {
                m("pop", reg_list(regs))
            } else {
                m(
                    if decrement_before { "ldmdb" } else { "ldm" },
                    format!(
                        "{}{}, {}",
                        reg(rn),
                        if writeback { "!" } else { "" },
                        reg_list(regs)
                    ),
                )
            }
        }
        Instr::Stm {
            rn,
            regs,
            writeback,
            decrement_before,
        } => {
            if rn == 13 && writeback && decrement_before {
                m("push", reg_list(regs))
            } else {
                m(
                    if decrement_before { "stmdb" } else { "stm" },
                    format!(
                        "{}{}, {}",
                        reg(rn),
                        if writeback { "!" } else { "" },
                        reg_list(regs)
                    ),
                )
            }
        }
        Instr::It { firstcond, mask } => {
            let low = mask.trailing_zeros().min(3);
            let mut name = String::from("it");
            for i in 0..(3 - low) {
                let bit = (mask >> (3 - i)) & 1;
                name.push(if bit == firstcond & 1 { 't' } else { 'e' });
            }
            (
                name,
                CONDITIONS
                    .get(usize::from(firstcond))
                    .copied()
                    .unwrap_or("")
                    .to_string(),
                None,
            )
        }
        Instr::Nop => m("nop", String::new()),
        Instr::Wfi => m("wfi", String::new()),
        Instr::Wfe => m("wfe", String::new()),
        Instr::Svc { imm: v } => m("svc", imm(u32::from(v))),
        Instr::Bkpt { imm: v } => m("bkpt", imm(u32::from(v))),
        Instr::Udf { imm: v } => m("udf", imm(u32::from(v))),
        Instr::Mrs { rd, sysreg: s } => m("mrs", format!("{}, {}", reg(rd), sysreg(s.0))),
        Instr::Msr { rn, sysreg: s, .. } => m("msr", format!("{}, {}", sysreg(s.0), reg(rn))),
        Instr::Cps { disable, i, f } => {
            let which = format!("{}{}", if i { "i" } else { "" }, if f { "f" } else { "" });
            m(if disable { "cpsid" } else { "cpsie" }, which)
        }
        Instr::Barrier => m("dmb", String::new()),
        Instr::Clrex => m("clrex", String::new()),
        Instr::Exclusive {
            op,
            rt,
            rd,
            rn,
            offset,
        } => {
            let name = format!("{op:?}").to_lowercase();
            let mem = if offset == 0 {
                format!("[{}]", reg(rn))
            } else {
                format!("[{}, {}]", reg(rn), imm(offset))
            };
            if op.is_store() {
                m(&name, format!("{}, {}, {mem}", reg(rd), reg(rt)))
            } else {
                m(&name, format!("{}, {mem}", reg(rt)))
            }
        }
        Instr::Fp(f) => render_fp(f),
    }
}

fn render_fp(f: FpInstr) -> (String, String, Option<u32>) {
    let m = |text: &str, ops: String| (text.to_string(), ops, None);
    match f {
        FpInstr::Arith { op, d, n, m: rm } => {
            let name = match op {
                FpArith::Vmla => "vmla",
                FpArith::Vmls => "vmls",
                FpArith::Vnmla => "vnmla",
                FpArith::Vnmls => "vnmls",
                FpArith::Vmul => "vmul",
                FpArith::Vnmul => "vnmul",
                FpArith::Vadd => "vadd",
                FpArith::Vsub => "vsub",
                FpArith::Vdiv => "vdiv",
                FpArith::Vfma => "vfma",
                FpArith::Vfms => "vfms",
                FpArith::Vfnma => "vfnma",
                FpArith::Vfnms => "vfnms",
            };
            m(
                &format!("{name}.f32"),
                format!("{}, {}, {}", s_reg(d), s_reg(n), s_reg(rm)),
            )
        }
        FpInstr::Unary { op, d, m: rm } => {
            let name = match op {
                FpUnary::Vmov => "vmov",
                FpUnary::Vabs => "vabs",
                FpUnary::Vneg => "vneg",
                FpUnary::Vsqrt => "vsqrt",
            };
            m(
                &format!("{name}.f32"),
                format!("{}, {}", s_reg(d), s_reg(rm)),
            )
        }
        FpInstr::MovImm { d, bits } => m(
            "vmov.f32",
            format!("{}, #{}", s_reg(d), f32::from_bits(bits)),
        ),
        FpInstr::Cmp {
            d,
            m: rm,
            signal_nan,
        } => {
            let name = if signal_nan { "vcmpe.f32" } else { "vcmp.f32" };
            m(
                name,
                format!("{}, {}", s_reg(d), rm.map_or("#0.0".to_string(), s_reg)),
            )
        }
        FpInstr::ToInt {
            signed,
            round_zero,
            d,
            m: rm,
        } => m(
            &format!(
                "vcvt{}.{}.f32",
                if round_zero { "" } else { "r" },
                if signed { "s32" } else { "u32" }
            ),
            format!("{}, {}", s_reg(d), s_reg(rm)),
        ),
        FpInstr::FromInt { signed, d, m: rm } => m(
            &format!("vcvt.f32.{}", if signed { "s32" } else { "u32" }),
            format!("{}, {}", s_reg(d), s_reg(rm)),
        ),
        FpInstr::Vmrs { rt } => m(
            "vmrs",
            format!(
                "{}, fpscr",
                if rt == 15 {
                    "apsr_nzcv".to_string()
                } else {
                    reg(rt)
                }
            ),
        ),
        FpInstr::Vmsr { rt } => m("vmsr", format!("fpscr, {}", reg(rt))),
        FpInstr::MovCore { to_core, rt, s } => {
            if to_core {
                m("vmov", format!("{}, {}", reg(rt), s_reg(s)))
            } else {
                m("vmov", format!("{}, {}", s_reg(s), reg(rt)))
            }
        }
        FpInstr::Ldr {
            first,
            words,
            rn,
            offset,
        } => {
            let target = if words == 2 {
                d_reg(first)
            } else {
                s_reg(first)
            };
            m(
                "vldr",
                format!("{target}, {}", indexed(rn, offset, IndexMode::Offset)),
            )
        }
        FpInstr::Str {
            first,
            words,
            rn,
            offset,
        } => {
            let target = if words == 2 {
                d_reg(first)
            } else {
                s_reg(first)
            };
            m(
                "vstr",
                format!("{target}, {}", indexed(rn, offset, IndexMode::Offset)),
            )
        }
        FpInstr::Ldm {
            first,
            words,
            rn,
            writeback,
            decrement_before,
        } => {
            let last = first + words.saturating_sub(1);
            let list = format!("{{{}-{}}}", s_reg(first), s_reg(last));
            if rn == 13 && writeback && !decrement_before {
                m("vpop", list)
            } else {
                m(
                    if decrement_before { "vldmdb" } else { "vldm" },
                    format!("{}{}, {list}", reg(rn), if writeback { "!" } else { "" }),
                )
            }
        }
        FpInstr::Stm {
            first,
            words,
            rn,
            writeback,
            decrement_before,
        } => {
            let last = first + words.saturating_sub(1);
            let list = format!("{{{}-{}}}", s_reg(first), s_reg(last));
            if rn == 13 && writeback && decrement_before {
                m("vpush", list)
            } else {
                m(
                    if decrement_before { "vstmdb" } else { "vstm" },
                    format!("{}{}, {list}", reg(rn), if writeback { "!" } else { "" }),
                )
            }
        }
        // Les formes rares (double précision, arrondis imposés, virgule fixe…) : le nom de la variante suffit.
        other => ("vfp".into(), format!("{other:?}").to_lowercase(), None),
    }
}

/// Le décodage d'une instruction à `address` : l'instruction, sa taille en octets et ses demi-mots.
fn fetch(
    read: &mut dyn FnMut(u32) -> Option<u16>,
    address: u32,
) -> Option<(Option<Instr>, u8, [u16; 2])> {
    let hw1 = read(address)?;
    if is_32bit(hw1) {
        let hw2 = read(address.wrapping_add(2))?;
        Some((decode32(hw1, hw2), 4, [hw1, hw2]))
    } else {
        Some((decode16(hw1), 2, [hw1, 0]))
    }
}

/// Désassemble `count` instructions à partir de `start`, ou moins si la mémoire n'est plus lisible. `read` rend le demi-mot
/// (petit-boutiste) à une adresse, ou `None`.
pub fn disassemble(
    read: &mut dyn FnMut(u32) -> Option<u16>,
    start: u32,
    count: usize,
) -> Vec<Line> {
    let mut lines = Vec::with_capacity(count);
    let mut address = start & !1;
    // Conditions des instructions encore à venir dans un bloc `it`.
    let mut pending: Vec<u8> = Vec::new();
    while lines.len() < count {
        let Some((instr, size, halfwords)) = fetch(read, address) else {
            break;
        };
        let in_it = !pending.is_empty();
        let condition = if in_it { Some(pending.remove(0)) } else { None };
        let (mnemonic, operands, target) = match &instr {
            Some(i) => render(i, address, u32::from(size), in_it),
            None if size == 2 => (".hword".to_string(), format!("{:#06x}", halfwords[0]), None),
            None => (
                ".word".to_string(),
                format!("{:#06x}, {:#06x}", halfwords[0], halfwords[1]),
                None,
            ),
        };
        let mut mnemonic = mnemonic;
        if let (Some(c), Some(i)) = (condition, &instr)
            && !matches!(i, Instr::BCond { .. } | Instr::It { .. })
        {
            mnemonic.push_str(CONDITIONS.get(usize::from(c)).copied().unwrap_or(""));
        }
        if let Some(Instr::It { firstcond, mask }) = instr {
            // La première instruction suivante prend `firstcond` ; les autres, `firstcond` ou son contraire.
            let low = mask.trailing_zeros().min(3);
            pending.push(firstcond);
            for i in 0..(3 - low) {
                let bit = (mask >> (3 - i)) & 1;
                pending.push(if bit == firstcond & 1 {
                    firstcond
                } else {
                    firstcond ^ 1
                });
            }
        }
        lines.push(Line {
            address,
            size,
            halfwords,
            mnemonic,
            operands,
            target,
        });
        address = address.wrapping_add(u32::from(size));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Désassemble des demi-mots placés à `base`.
    fn dis(base: u32, code: &[u16]) -> Vec<(String, Option<u32>)> {
        let mut read = |a: u32| code.get(((a - base) / 2) as usize).copied();
        disassemble(&mut read, base, code.len())
            .into_iter()
            .map(|l| {
                (
                    format!("{} {}", l.mnemonic, l.operands).trim().to_string(),
                    l.target,
                )
            })
            .collect()
    }

    fn first(code: &[u16]) -> String {
        let mut read = |a: u32| code.get((a / 2) as usize).copied();
        let line = &disassemble(&mut read, 0, 1)[0];
        format!("{} {}", line.mnemonic, line.operands)
            .trim()
            .to_string()
    }

    #[test]
    fn common_16_bit_instructions() {
        assert_eq!(first(&[0xb510]), "push {r4, lr}");
        assert_eq!(first(&[0xbd10]), "pop {r4, pc}");
        assert_eq!(first(&[0x4770]), "bx lr");
        assert_eq!(first(&[0x2005]), "movs r0, #5");
        assert_eq!(first(&[0x2a0a]), "cmp r2, #10");
        assert_eq!(first(&[0x6803]), "ldr r3, [r0]");
        assert_eq!(first(&[0x6043]), "str r3, [r0, #4]");
        assert_eq!(first(&[0x1840]), "adds r0, r0, r1");
        assert_eq!(first(&[0xdf01]), "svc #1");
        assert_eq!(first(&[0xbf00]), "nop");
    }

    #[test]
    fn branches_show_their_target() {
        // `beq` sur lui-même, puis `b` en avant, puis `bl` de 32 bits.
        let lines = dis(0x1000, &[0xd0fe]);
        assert_eq!(lines[0], ("beq 0x1000".to_string(), Some(0x1000)));
        let lines = dis(0x1000, &[0xe000]);
        assert_eq!(lines[0], ("b 0x1004".to_string(), Some(0x1004)));
        let lines = dis(0x2000, &[0xf000, 0xf800]);
        assert_eq!(lines[0], ("bl 0x2004".to_string(), Some(0x2004)));
    }

    #[test]
    fn literal_loads_point_at_their_word() {
        let lines = dis(0x1000, &[0x4b02]);
        assert_eq!(lines[0], ("ldr r3, [pc, #8]".to_string(), Some(0x100c)));
    }

    #[test]
    fn it_blocks_give_their_suffixes_to_the_next_instructions() {
        // ite eq ; moveq r0, #1 ; movne r0, #2
        let lines = dis(0, &[0xbf0c, 0x2001, 0x2002]);
        assert_eq!(lines[0].0, "ite eq");
        assert_eq!(lines[1].0, "moveq r0, #1");
        assert_eq!(lines[2].0, "movne r0, #2");
        // Après le bloc, plus de suffixe.
        let lines = dis(0, &[0xbf08, 0x2001, 0x2002]);
        assert_eq!(lines[0].0, "it eq");
        assert_eq!(lines[1].0, "moveq r0, #1");
        assert_eq!(lines[2].0, "movs r0, #2");
    }

    #[test]
    fn wide_instructions_take_four_bytes() {
        let mut read = |a: u32| [0xe92du16, 0x4ff0][(a / 2) as usize % 2].into();
        let lines = disassemble(&mut read, 0, 1);
        assert_eq!(lines[0].size, 4);
        assert_eq!(
            format!("{} {}", lines[0].mnemonic, lines[0].operands),
            "push {r4, r5, r6, r7, r8, r9, r10, r11, lr}"
        );
    }

    #[test]
    fn unknown_encodings_are_data_and_unreadable_memory_ends_the_listing() {
        let lines = dis(0, &[0xde01]);
        assert_eq!(lines[0].0, "udf #1");
        let mut read = |a: u32| (a < 4).then_some(0xbf00u16);
        assert_eq!(disassemble(&mut read, 0, 10).len(), 2);
    }
}
