//! Des instructions sur des milliers de valeurs tirées au hasard, comparées à un calcul de référence écrit en une ligne :
//! résultats et indicateurs `N Z C V` de l'arithmétique, des opérations logiques, des décalages, de la multiplication et de
//! la division ; et le décodage ou le désassemblage de n'importe quel demi-mot ne doit jamais faire paniquer.
//!
//! Chaque propriété tourne sur peu de cas (`CASES`) pour rester rapide : la suite de tests est déjà longue.

use proptest::prelude::*;

use super::tests::run;
use super::{Cpu, decode16, decode32, disassemble, is_32bit};

/// Nombre de tirages par propriété.
const CASES: u32 = 256;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: CASES,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// Un mot de 32 bits : le plus souvent tiré au hasard, mais aussi les valeurs où les cas limites se cachent (0, 1, les extrêmes
/// signés et non signés, leurs voisins) ; sans elles, un tirage uniforme ne tombe presque jamais sur une somme égale à `u32::MAX`.
fn word() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => any::<u32>(),
        3 => prop::sample::select(vec![
            0u32, 1, 2, 31, 32, 33, 0x7fff_ffff, 0x8000_0000, 0x8000_0001, 0xffff_fffe, 0xffff_ffff,
        ]),
        1 => 0u32..300,
    ]
}

type Flags = (bool, bool, bool, bool);

fn flags(cpu: &Cpu) -> Flags {
    (cpu.regs.n(), cpu.regs.z(), cpu.regs.c(), cpu.regs.v())
}

/// `a + b + carry` avec la retenue et le débordement signé, comme la pseudo-fonction `AddWithCarry` du manuel d'ARM.
fn add_with_carry(a: u32, b: u32, carry: bool) -> (u32, bool, bool) {
    let total = u64::from(a) + u64::from(b) + u64::from(carry);
    let result = total as u32;
    let overflow = ((a ^ result) & (b ^ result)) >> 31 == 1;
    (result, total > u64::from(u32::MAX), overflow)
}

fn nz(result: u32) -> (bool, bool) {
    (result >> 31 == 1, result == 0)
}

/// Exécute une instruction de 16 bits avec `r0 = a`, `r1 = b` et les indicateurs de départ donnés.
fn run1(hw: u16, a: u32, b: u32, start: Flags) -> Cpu {
    let (cpu, _, _) = run(
        &[hw],
        |cpu| {
            cpu.regs.r[0] = a;
            cpu.regs.r[1] = b;
            cpu.regs.set_n(start.0);
            cpu.regs.set_z(start.1);
            cpu.regs.set_c(start.2);
            cpu.regs.set_v(start.3);
        },
        1,
    );
    cpu
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn adds_matches_the_reference(a in word(), b in word(), n: bool, z: bool, c: bool, v: bool) {
        // adds r2, r0, r1
        let cpu = run1(0x1842, a, b, (n, z, c, v));
        let (result, carry, overflow) = add_with_carry(a, b, false);
        prop_assert_eq!(cpu.regs.r[2], result);
        prop_assert_eq!(flags(&cpu), (nz(result).0, nz(result).1, carry, overflow));
    }

    #[test]
    fn subs_and_cmp_match_the_reference(a in word(), b in word()) {
        let (result, carry, overflow) = add_with_carry(a, !b, true);
        let expected = (nz(result).0, nz(result).1, carry, overflow);
        // subs r2, r0, r1
        let cpu = run1(0x1a42, a, b, (false, false, false, false));
        prop_assert_eq!(cpu.regs.r[2], result);
        prop_assert_eq!(flags(&cpu), expected);
        // cmp r0, r1 : mêmes indicateurs, r0 intact
        let cpu = run1(0x4288, a, b, (false, false, false, false));
        prop_assert_eq!(cpu.regs.r[0], a);
        prop_assert_eq!(flags(&cpu), expected);
    }

    #[test]
    fn adcs_and_sbcs_use_the_carry_in(a in word(), b in word(), carry_in: bool) {
        // adcs r0, r1
        let cpu = run1(0x4148, a, b, (false, false, carry_in, false));
        let (result, carry, overflow) = add_with_carry(a, b, carry_in);
        prop_assert_eq!(cpu.regs.r[0], result);
        prop_assert_eq!(flags(&cpu), (nz(result).0, nz(result).1, carry, overflow));
        // sbcs r0, r1
        let cpu = run1(0x4188, a, b, (false, false, carry_in, false));
        let (result, carry, overflow) = add_with_carry(a, !b, carry_in);
        prop_assert_eq!(cpu.regs.r[0], result);
        prop_assert_eq!(flags(&cpu), (nz(result).0, nz(result).1, carry, overflow));
    }

    #[test]
    fn logic_sets_n_and_z_and_leaves_c_and_v(a in word(), b in word(), c: bool, v: bool) {
        // ands, eors, orrs, bics (r0 = r0 op r1)
        for (hw, result) in [(0x4008u16, a & b), (0x4048, a ^ b), (0x4308, a | b), (0x4388, a & !b)] {
            let cpu = run1(hw, a, b, (false, false, c, v));
            prop_assert_eq!(cpu.regs.r[0], result, "instruction {:#06x}", hw);
            prop_assert_eq!(flags(&cpu), (nz(result).0, nz(result).1, c, v), "instruction {:#06x}", hw);
        }
        // mvns r0, r1
        let cpu = run1(0x43c8, a, b, (false, false, c, v));
        prop_assert_eq!(cpu.regs.r[0], !b);
        prop_assert_eq!(flags(&cpu), (nz(!b).0, nz(!b).1, c, v));
    }

    #[test]
    fn shifts_by_a_register_match_the_manual(a in word(), b in word(), c: bool, v: bool) {
        let amount = b & 0xff;
        // lsls r0, r1
        let (result, carry) = match amount {
            0 => (a, c),
            1..=31 => (a << amount, (a >> (32 - amount)) & 1 == 1),
            32 => (0, a & 1 == 1),
            _ => (0, false),
        };
        let cpu = run1(0x4088, a, b, (false, false, c, v));
        prop_assert_eq!(cpu.regs.r[0], result, "lsls par {}", amount);
        prop_assert_eq!(flags(&cpu), (nz(result).0, nz(result).1, carry, v), "lsls par {}", amount);
        // lsrs r0, r1
        let (result, carry) = match amount {
            0 => (a, c),
            1..=31 => (a >> amount, (a >> (amount - 1)) & 1 == 1),
            32 => (0, a >> 31 == 1),
            _ => (0, false),
        };
        let cpu = run1(0x40c8, a, b, (false, false, c, v));
        prop_assert_eq!(cpu.regs.r[0], result, "lsrs par {}", amount);
        prop_assert_eq!(flags(&cpu), (nz(result).0, nz(result).1, carry, v), "lsrs par {}", amount);
        // asrs r0, r1
        let (result, carry) = match amount {
            0 => (a, c),
            1..=31 => (((a as i32) >> amount) as u32, (a >> (amount - 1)) & 1 == 1),
            _ => (((a as i32) >> 31) as u32, a >> 31 == 1),
        };
        let cpu = run1(0x4108, a, b, (false, false, c, v));
        prop_assert_eq!(cpu.regs.r[0], result, "asrs par {}", amount);
        prop_assert_eq!(flags(&cpu), (nz(result).0, nz(result).1, carry, v), "asrs par {}", amount);
    }

    #[test]
    fn muls_wraps_and_sets_n_and_z(a in word(), b in word(), c: bool, v: bool) {
        // muls r0, r1, r0
        let cpu = run1(0x4348, a, b, (false, false, c, v));
        let result = a.wrapping_mul(b);
        prop_assert_eq!(cpu.regs.r[0], result);
        prop_assert_eq!(flags(&cpu), (nz(result).0, nz(result).1, c, v));
    }

    #[test]
    fn udiv_and_sdiv_follow_the_manual(a in word(), b in word()) {
        // udiv r2, r0, r1 ; sdiv r2, r0, r1 (la division par zéro donne 0, le seul débordement possible donne i32::MIN)
        let udiv = a.checked_div(b).unwrap_or(0);
        let sdiv = if b == 0 { 0 } else { (a as i32).wrapping_div(b as i32) as u32 };
        for (hw1, expected) in [(0xfbb0u16, udiv), (0xfb90, sdiv)] {
            let (cpu, ..) = run(
                &[hw1, 0xf2f1],
                |cpu| {
                    cpu.regs.r[0] = a;
                    cpu.regs.r[1] = b;
                },
                1,
            );
            prop_assert_eq!(cpu.regs.r[2], expected, "{:#06x} : {} / {}", hw1, a, b);
        }
    }

    #[test]
    fn no_encoding_makes_the_decoder_or_the_disassembler_panic(hw1: u16, hw2: u16) {
        if is_32bit(hw1) {
            let _ = decode32(hw1, hw2);
        } else {
            let _ = decode16(hw1);
        }
        let mut read = |address: u32| match address {
            0 => Some(hw1),
            2 => Some(hw2),
            _ => None,
        };
        let _ = disassemble(&mut read, 0, 1);
    }
}
