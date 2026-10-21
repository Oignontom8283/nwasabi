//! Tests du processeur sur de petits programmes écrits en codes d'instructions.

use super::*;

/// Mémoire plate de 64 Kio commençant à l'adresse 0, pour exécuter de petits programmes.
struct Flat(Vec<u8>);

impl Flat {
    fn new() -> Self {
        Self(vec![0; 0x10000])
    }

    fn with_halfwords(code: &[u16]) -> Self {
        let mut m = Self::new();
        for (i, hw) in code.iter().enumerate() {
            m.0[0x100 + 2 * i..0x102 + 2 * i].copy_from_slice(&hw.to_le_bytes());
        }
        m
    }

    fn fault(address: u32, write: bool, width: Width) -> BusFault {
        BusFault {
            address,
            write,
            width,
            kind: FaultKind::Unmapped,
        }
    }

    fn get(&self, address: u32, n: usize, write: bool, width: Width) -> Result<&[u8], BusFault> {
        self.0
            .get(address as usize..address as usize + n)
            .ok_or(Self::fault(address, write, width))
    }

    fn put(&mut self, address: u32, bytes: &[u8], width: Width) -> Result<(), BusFault> {
        let end = address as usize + bytes.len();
        self.0
            .get_mut(address as usize..end)
            .ok_or(Self::fault(address, true, width))?
            .copy_from_slice(bytes);
        Ok(())
    }
}

impl Bus for Flat {
    fn read8(&mut self, a: u32) -> Result<u8, BusFault> {
        Ok(self.get(a, 1, false, Width::U8)?[0])
    }
    fn read16(&mut self, a: u32) -> Result<u16, BusFault> {
        let b = self.get(a, 2, false, Width::U16)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn read32(&mut self, a: u32) -> Result<u32, BusFault> {
        let b = self.get(a, 4, false, Width::U32)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn write8(&mut self, a: u32, v: u8) -> Result<(), BusFault> {
        self.put(a, &[v], Width::U8)
    }
    fn write16(&mut self, a: u32, v: u16) -> Result<(), BusFault> {
        self.put(a, &v.to_le_bytes(), Width::U16)
    }
    fn write32(&mut self, a: u32, v: u32) -> Result<(), BusFault> {
        self.put(a, &v.to_le_bytes(), Width::U32)
    }
}

/// Hooks qui enregistrent les `svc` et rendent la main après chacun.
#[derive(Default)]
struct RecordingHooks {
    svcs: Vec<u8>,
}

impl Hooks for RecordingHooks {
    fn on_svc(&mut self, imm8: u8, _cpu: &mut Cpu) -> SvcAction {
        self.svcs.push(imm8);
        SvcAction::HandledAndStop
    }
}

const START: u32 = 0x100;

/// Exécute `count` instructions du programme et renvoie le processeur.
fn run(code: &[u16], setup: impl FnOnce(&mut Cpu), count: u64) -> (Cpu, Flat, Stop) {
    let mut bus = Flat::with_halfwords(code);
    let mut cpu = Cpu::new();
    cpu.regs.pc = START;
    cpu.regs.sp = 0x8000;
    setup(&mut cpu);
    let stop = cpu.run(&mut bus, &mut (), count);
    (cpu, bus, stop)
}

impl Hooks for () {}

#[test]
fn small_arithmetic_program() {
    // movs r0, #6 ; movs r1, #7 ; muls r0, r1, r0
    let (cpu, _, stop) = run(&[0x2006, 0x2107, 0x4348], |_| {}, 3);
    assert_eq!(stop, Stop::Budget);
    assert_eq!(cpu.regs.r[0], 42);
    assert_eq!(cpu.regs.pc, START + 6);
}

#[test]
fn adds_sets_flags_outside_an_it_block() {
    // movs r0, #0xff ; adds r0, #1 ; (0xff + 1 = 0x100, pas de retenue sur 32 bits)
    let (cpu, ..) = run(&[0x20ff, 0x3001], |_| {}, 2);
    assert_eq!(cpu.regs.r[0], 0x100);
    assert!(!cpu.regs.z() && !cpu.regs.c());
    // subs r0, r0, r0 -> 0, Z et C levés.
    let (cpu, ..) = run(&[0x1a00], |c| c.regs.r[0] = 5, 1);
    assert!(cpu.regs.z() && cpu.regs.c() && !cpu.regs.n());
}

#[test]
fn it_block_runs_the_instructions_whose_condition_holds() {
    // cmp r0, #0 ; ite eq ; moveq r1, #5 ; movne r1, #9
    let program = [0x2800, 0xbf0c, 0x2105, 0x2109];
    let (cpu, ..) = run(&program, |c| c.regs.r[0] = 0, 4);
    assert_eq!(cpu.regs.r[1], 5);
    let (cpu, ..) = run(&program, |c| c.regs.r[0] = 1, 4);
    assert_eq!(cpu.regs.r[1], 9);
    // Hors du bloc, `movs` met les indicateurs à jour ; dedans, non (ici Z reste celui du cmp).
    let (cpu, ..) = run(&program, |c| c.regs.r[0] = 0, 3);
    assert!(
        cpu.regs.z(),
        "Z du cmp r0,#0 conservé : movs dans l'IT ne touche pas aux indicateurs"
    );
    // `ite eq` valait 0b0000_1100 ; après la première instruction (« then »), il reste le « else ».
    assert_eq!(cpu.regs.it_state(), 0b0001_1000);
}

#[test]
fn bl_and_bx_lr_form_a_call() {
    // 0x100: bl +4 (vers 0x108) ; 0x104: movs r2, #1 ; 0x106: b . ; 0x108: movs r3, #7 ; 0x10a: bx lr
    // bl offset: cible = 0x108 = pc4 (0x104) + 4 → offset 4 : hw1 = f000, hw2 = f802.
    let program = [0xf000, 0xf802, 0x2201, 0xe7fe, 0x2307, 0x4770];
    let (cpu, ..) = run(&program, |_| {}, 4);
    assert_eq!(cpu.regs.r[3], 7);
    assert_eq!(cpu.regs.r[2], 1);
    assert_eq!(cpu.regs.lr, (START + 4) | 1);
}

#[test]
fn push_and_pop_save_and_restore_registers() {
    // push {r4, lr} ; movs r4, #9 ; pop {r4, pc}
    let program = [0xb510, 0x2409, 0xbd10];
    let (cpu, bus, _) = run(
        &program,
        |c| {
            c.regs.r[4] = 0x1234;
            c.regs.lr = 0x201;
        },
        2,
    );
    assert_eq!(cpu.regs.sp, 0x8000 - 8);
    assert_eq!(cpu.regs.r[4], 9);
    assert_eq!(
        u32::from_le_bytes(bus.0[0x7ff8..0x7ffc].try_into().unwrap()),
        0x1234
    );
    // Après le pop : r4 restauré et pc = lr (bit 0 ignoré).
    let (cpu, ..) = run(
        &program,
        |c| {
            c.regs.r[4] = 0x1234;
            c.regs.lr = 0x201;
        },
        3,
    );
    assert_eq!(
        (cpu.regs.r[4], cpu.regs.pc, cpu.regs.sp),
        (0x1234, 0x200, 0x8000)
    );
}

#[test]
fn pc_relative_load_reads_the_literal_pool() {
    // ldr r0, [pc, #0] : lit à Align(pc + 4, 4) + 0.
    let mut bus = Flat::with_halfwords(&[0x4800]);
    bus.0[0x104..0x108].copy_from_slice(&0xcafe_f00du32.to_le_bytes());
    let mut cpu = Cpu::new();
    cpu.regs.pc = START;
    cpu.step(&mut bus, &mut ()).unwrap();
    assert_eq!(cpu.regs.r[0], 0xcafe_f00d);
}

#[test]
fn thumb2_movw_movt_build_a_constant() {
    // movw r3, #0xabcd ; movt r3, #0x1234 (valeurs de l'assembleur GNU)
    let (cpu, ..) = run(&[0xf64a, 0x33cd, 0xf2c1, 0x2334], |_| {}, 2);
    assert_eq!(cpu.regs.r[3], 0x1234_abcd);
    assert_eq!(cpu.regs.pc, START + 8);
}

#[test]
fn thirty_two_bit_bl_reaches_its_target() {
    // bl vers 0x3026 depuis 0x100 : offset = 0x3026 - 0x104 = 0x2f22 ; encodage calculé à la main
    // par l'assembleur GNU pour le même décalage relatif (voir tests de relocation).
    let offset = 0x3026i32 - 0x104;
    let (hw1, hw2) = bl_encoding(offset);
    let (cpu, ..) = run(&[hw1, hw2], |_| {}, 1);
    assert_eq!(cpu.regs.pc, 0x3026);
    assert_eq!(cpu.regs.lr, (START + 4) | 1);
}

/// Encodage de `bl` pour un décalage donné (champs S, J1, J2, imm10, imm11).
fn bl_encoding(offset: i32) -> (u16, u16) {
    let v = offset as u32;
    let s = (v >> 24) & 1;
    let (i1, i2) = ((v >> 23) & 1, (v >> 22) & 1);
    let (j1, j2) = (!(i1 ^ s) & 1, !(i2 ^ s) & 1);
    let hw1 = 0xf000 | (s << 10) | ((v >> 12) & 0x3ff);
    let hw2 = 0xd000 | (j1 << 13) | (j2 << 11) | ((v >> 1) & 0x7ff);
    (hw1 as u16, hw2 as u16)
}

#[test]
fn svc_is_given_to_the_hooks() {
    // svc #0x2a
    let mut bus = Flat::with_halfwords(&[0xdf2a, 0x2001]);
    let mut cpu = Cpu::new();
    cpu.regs.pc = START;
    let mut hooks = RecordingHooks::default();
    assert_eq!(cpu.step(&mut bus, &mut hooks), Err(Stop::Requested));
    assert_eq!(hooks.svcs, [0x2a]);
    // L'instruction suivante est celle d'après le svc.
    assert_eq!(cpu.regs.pc, START + 2);
}

#[test]
fn unknown_encodings_stop_with_their_address() {
    // 0xe800 0x0000 : srs/rfe, non géré.
    let (_, _, stop) = run(&[0xe800, 0x0000], |_| {}, 1);
    assert_eq!(
        stop,
        Stop::Unimplemented {
            pc: START,
            halfword: 0xe800
        }
    );
    // Un udf lève une UsageFault.
    let (_, _, stop) = run(&[0xde00], |_| {}, 1);
    assert_eq!(
        stop,
        Stop::UsageFault {
            pc: START,
            kind: UsageFault::UndefinedInstruction
        }
    );
}

#[test]
fn bus_faults_stop_the_processor() {
    // ldr r0, [r1] avec r1 hors de la mémoire.
    let (_, _, stop) = run(&[0x6808], |c| c.regs.r[1] = 0x4000_0000, 1);
    assert!(matches!(stop, Stop::BusFault(f) if f.address == 0x4000_0000 && !f.write));
}

#[test]
fn bx_to_an_even_address_is_an_invalid_state() {
    let (_, _, stop) = run(&[0x4708], |c| c.regs.r[1] = 0x200, 1);
    assert_eq!(
        stop,
        Stop::UsageFault {
            pc: START,
            kind: UsageFault::InvalidState
        }
    );
}

/// Hooks qui laissent le processeur lever l'exception (comportement par défaut).
struct Raise;
impl Hooks for Raise {}

#[test]
fn svc_raised_as_an_exception_stacks_the_context_and_returns() {
    // Table des vecteurs à 0x0 : SVCall (n° 11) -> gestionnaire à 0x300 (Thumb).
    // 0x100: svc #1 ; 0x102: movs r5, #5
    // 0x300: movs r6, #6 ; 0x302: bx lr
    let mut bus = Flat::with_halfwords(&[0xdf01, 0x2505]);
    bus.0[44..48].copy_from_slice(&0x301u32.to_le_bytes());
    bus.0[0x300..0x304].copy_from_slice(&[0x06, 0x26, 0x70, 0x47]);
    let mut cpu = Cpu::new();
    cpu.regs.pc = START;
    cpu.regs.sp = 0x8000;
    cpu.msp = 0x8000;
    cpu.regs.r[0] = 0x11;
    cpu.regs.r[12] = 0x22;
    cpu.regs.lr = 0x1235;
    cpu.step(&mut bus, &mut Raise).unwrap(); // svc : entrée d'exception
    assert!(cpu.handler_mode);
    assert_eq!(cpu.regs.pc, 0x300);
    assert_eq!(cpu.regs.sp, 0x8000 - 0x20);
    assert_eq!(cpu.regs.lr, 0xffff_fff9);
    assert_eq!(cpu.regs.ipsr(), 11);
    // Contexte empilé : r0, r1, r2, r3, r12, lr, adresse de retour, xPSR.
    let word = |bus: &Flat, i: usize| {
        u32::from_le_bytes(bus.0[0x7fe0 + 4 * i..0x7fe4 + 4 * i].try_into().unwrap())
    };
    assert_eq!(
        (word(&bus, 0), word(&bus, 4), word(&bus, 5), word(&bus, 6)),
        (0x11, 0x22, 0x1235, 0x102)
    );
    cpu.step(&mut bus, &mut Raise).unwrap(); // movs r6, #6
    cpu.step(&mut bus, &mut Raise).unwrap(); // bx lr : retour d'exception
    assert!(!cpu.handler_mode);
    assert_eq!(
        (cpu.regs.pc, cpu.regs.sp, cpu.regs.lr, cpu.regs.r[0]),
        (0x102, 0x8000, 0x1235, 0x11)
    );
    assert_eq!(cpu.regs.r[6], 6);
    cpu.step(&mut bus, &mut Raise).unwrap(); // movs r5, #5
    assert_eq!(cpu.regs.r[5], 5);
}

#[test]
fn exception_entry_aligns_the_stack_on_eight_bytes() {
    let mut bus = Flat::with_halfwords(&[0xdf01]);
    bus.0[44..48].copy_from_slice(&0x301u32.to_le_bytes());
    bus.0[0x300..0x302].copy_from_slice(&[0x70, 0x47]);
    let mut cpu = Cpu::new();
    cpu.regs.pc = START;
    cpu.regs.sp = 0x8004; // pas alignée sur 8
    cpu.msp = 0x8004;
    cpu.regs.lr = 0x1235;
    cpu.step(&mut bus, &mut Raise).unwrap();
    assert_eq!(cpu.regs.sp & 7, 0);
    cpu.step(&mut bus, &mut Raise).unwrap(); // bx lr
    assert_eq!(
        cpu.regs.sp, 0x8004,
        "le mot de remplissage est retiré au retour"
    );
}
