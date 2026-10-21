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

