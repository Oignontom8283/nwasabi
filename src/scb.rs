//! Registres de faute du Cortex-M7 (bloc de contrôle du système, SCB), simulés.
//!
//! nwasabi s'arrête quand l'application fait une faute au lieu d'exécuter un gestionnaire d'exception, mais il remplit les
//! registres que le processeur réel remplirait : un débogueur qui les lit (par l'éditeur de mémoire, ou plus tard par GDB)
//! voit un plantage ordinaire, avec la cause et l'adresse en cause.
//!
//! Les registres sont de la mémoire en lecture seule pour l'application (zone « scs » à `SCS_BASE`) ; seul l'hôte les écrit.

use crate::cpu::{FaultKind, UsageFault};
use crate::emulator::Fault;
use crate::memory::{Memory, MemoryError};

/// Début de la zone simulée : l'espace de contrôle du système (SCS), dont le SCB occupe 0xe000ed00 et suivants.
pub const SCS_BASE: u32 = 0xe000_e000;
/// Taille de la zone (une page ; les zones de mémoire sont alignées sur 4 Kio).
pub const SCS_SIZE: u32 = 0x1000;

pub const CPUID: u32 = 0xe000_ed00;
pub const ICSR: u32 = 0xe000_ed04;
pub const SHCSR: u32 = 0xe000_ed24;
pub const CFSR: u32 = 0xe000_ed28;
pub const HFSR: u32 = 0xe000_ed2c;
pub const DFSR: u32 = 0xe000_ed30;
pub const MMFAR: u32 = 0xe000_ed34;
pub const BFAR: u32 = 0xe000_ed38;

/// Cortex-M7 r1p0 : fabricant ARM (0x41), architecture ARMv7-M (0xF), partie 0xC27, révision 0.
const CPUID_VALUE: u32 = 0x411f_c270;

// CFSR : MMFSR (bits 0 à 7), BFSR (8 à 15), UFSR (16 à 31).
const IACCVIOL: u32 = 1 << 0;
const DACCVIOL: u32 = 1 << 1;
const MMARVALID: u32 = 1 << 7;
const IBUSERR: u32 = 1 << 8;
const PRECISERR: u32 = 1 << 9;
const BFARVALID: u32 = 1 << 15;
const UNDEFINSTR: u32 = 1 << 16;
const INVSTATE: u32 = 1 << 17;
const UNALIGNED: u32 = 1 << 24;
/// HFSR : la faute a été « escaladée » en HardFault, faute de gestionnaire dédié.
const FORCED: u32 = 1 << 30;
/// ICSR : numéro de l'exception active (3 : HardFault).
const HARDFAULT: u32 = 3;
/// DFSR : instruction `bkpt`.
const DFSR_BKPT: u32 = 1 << 1;

/// Valeurs de départ : seul l'identifiant n'est pas nul.
pub fn reset(memory: &mut Memory) -> Result<(), MemoryError> {
    memory.write_bytes(SCS_BASE, &[0; SCS_SIZE as usize])?;
    put(memory, CPUID, CPUID_VALUE)
}

fn get(memory: &mut Memory, address: u32) -> u32 {
    memory.read_u32(address).unwrap_or(0)
}

fn put(memory: &mut Memory, address: u32, value: u32) -> Result<(), MemoryError> {
    memory.write_bytes(address, &value.to_le_bytes())
}

/// Enregistre une faute de l'application. Les bits de CFSR, de HFSR et de DFSR se cumulent comme sur le processeur
/// (ils ne s'effacent qu'à la relance de l'application). `pc` est l'instruction fautive.
pub fn record(memory: &mut Memory, fault: &Fault, pc: u32) {
    let _ = record_inner(memory, fault, pc);
}

fn record_inner(memory: &mut Memory, fault: &Fault, pc: u32) -> Result<(), MemoryError> {
    let mut cfsr = 0;
    let mut dfsr = 0;
    let mut address: Option<(u32, bool)> = None; // adresse, et vrai si c'est MMFAR (sinon BFAR)
    match fault {
        Fault::Bus(bus) => {
            let fetch = !bus.write && bus.address & !1 == pc & !1;
            match (bus.kind, fetch) {
                (FaultKind::AccessDenied, true) => cfsr |= IACCVIOL,
                (FaultKind::AccessDenied, false) => {
                    cfsr |= DACCVIOL | MMARVALID;
                    address = Some((bus.address, true));
                }
                (_, true) => cfsr |= IBUSERR,
                (_, false) => {
                    cfsr |= PRECISERR | BFARVALID;
                    address = Some((bus.address, false));
                }
            }
        }
        Fault::Unimplemented { .. } => cfsr |= UNDEFINSTR,
        Fault::UsageFault { kind, .. } => {
            cfsr |= match kind {
                UsageFault::UndefinedInstruction => UNDEFINSTR,
                UsageFault::InvalidState => INVSTATE,
                UsageFault::Unaligned => UNALIGNED,
            };
        }
        Fault::Breakpoint { .. } => dfsr |= DFSR_BKPT,
        // Ni faute du processeur, ni registre à remplir.
        Fault::WaitForInterrupt | Fault::UnknownSvc(_) => return Ok(()),
    }
    if cfsr != 0 {
        let old = get(memory, CFSR);
        put(memory, CFSR, old | cfsr)?;
        let hfsr = get(memory, HFSR);
        put(memory, HFSR, hfsr | FORCED)?;
        let icsr = get(memory, ICSR);
        put(memory, ICSR, (icsr & !0x1ff) | HARDFAULT)?;
    }
    if dfsr != 0 {
        let old = get(memory, DFSR);
        put(memory, DFSR, old | dfsr)?;
    }
    if let Some((value, is_mem)) = address {
        put(memory, if is_mem { MMFAR } else { BFAR }, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::{BusFault, Width};
    use crate::memory::{Perms, RegionKind, RegionSpec};

    fn memory() -> Memory {
        let mut m = Memory::new();
        m.add_region(RegionSpec {
            name: "scs".into(),
            base: SCS_BASE,
            size: SCS_SIZE,
            perms: Perms::READ_ONLY,
            kind: RegionKind::Reserved,
            fill: 0,
        })
        .unwrap();
        reset(&mut m).unwrap();
        m
    }

    fn bus(address: u32, write: bool, kind: FaultKind) -> Fault {
        Fault::Bus(BusFault {
            address,
            write,
            width: Width::U32,
            kind,
        })
    }

    #[test]
    fn data_accesses_to_nothing_are_precise_bus_faults_with_their_address() {
        let mut m = memory();
        record(&mut m, &bus(0x10, false, FaultKind::Unmapped), 0x9000_0000);
        assert_eq!(get(&mut m, CFSR), PRECISERR | BFARVALID);
        assert_eq!(get(&mut m, BFAR), 0x10);
        assert_eq!(get(&mut m, HFSR), FORCED);
        assert_eq!(get(&mut m, ICSR) & 0x1ff, HARDFAULT);
    }

    #[test]
    fn forbidden_accesses_are_memory_management_faults() {
        let mut m = memory();
        record(
            &mut m,
            &bus(0x9000_0000, true, FaultKind::AccessDenied),
            0x2002_0000,
        );
        assert_eq!(get(&mut m, CFSR), DACCVIOL | MMARVALID);
        assert_eq!(get(&mut m, MMFAR), 0x9000_0000);
    }

    #[test]
    fn fetching_instructions_from_nowhere_or_from_forbidden_memory() {
        let mut m = memory();
        record(&mut m, &bus(0x1000, false, FaultKind::Unmapped), 0x1000);
        assert_eq!(get(&mut m, CFSR), IBUSERR);
        let mut m = memory();
        record(
            &mut m,
            &bus(0x3000_0000, false, FaultKind::AccessDenied),
            0x3000_0000,
        );
        assert_eq!(get(&mut m, CFSR), IACCVIOL);
    }

    #[test]
    fn usage_faults_set_their_bit_and_faults_accumulate() {
        let mut m = memory();
        record(
            &mut m,
            &Fault::UsageFault {
                pc: 0,
                kind: UsageFault::UndefinedInstruction,
            },
            0,
        );
        record(
            &mut m,
            &Fault::UsageFault {
                pc: 0,
                kind: UsageFault::Unaligned,
            },
            0,
        );
        assert_eq!(get(&mut m, CFSR), UNDEFINSTR | UNALIGNED);
        record(&mut m, &Fault::Unimplemented { pc: 0, halfword: 0 }, 0);
        assert_eq!(get(&mut m, CFSR), UNDEFINSTR | UNALIGNED);
    }

    #[test]
    fn breakpoints_set_the_debug_status_and_other_stops_change_nothing() {
        let mut m = memory();
        record(&mut m, &Fault::Breakpoint { imm8: 0xab }, 0);
        assert_eq!(get(&mut m, DFSR), DFSR_BKPT);
        assert_eq!(get(&mut m, CFSR), 0);
        record(&mut m, &Fault::UnknownSvc(9), 0);
        record(&mut m, &Fault::WaitForInterrupt, 0);
        assert_eq!(get(&mut m, CFSR), 0);
        assert_eq!(get(&mut m, HFSR), 0);
    }
}
