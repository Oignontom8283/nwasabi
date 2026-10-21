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

