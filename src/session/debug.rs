//! Aides au débogage : noms de fonctions et pile d'appels.

use crate::cpu::{Instr, decode16, decode32, is_32bit};
use crate::emulator::Emulator;
use crate::json::{Value, ValueExt};

/// Nombre maximal de mots de pile examinés.
const MAX_WORDS: u32 = 8192;

/// « fonction+0x10 » pour une adresse du code, ou `None`.
pub fn symbol_text(emu: &Emulator, address: u32) -> Option<String> {
    let (name, _, offset) = emu.symbol_at(address)?;
    Some(if offset == 0 {
        name.to_owned()
    } else {
        format!("{name}+{offset:#x}")
    })
}

/// L'instruction d'appel qui précède `ret` (adresse de retour, bit 0 retiré) : sa taille, et la cible d'un `bl`.
fn call_before(emu: &mut Emulator, ret: u32) -> Option<(u32, Option<u32>)> {
    let mut half = |a: u32| {
        let bytes = emu.device.memory.read_bytes(a, 2).ok()?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    };
    if let (Some(hw1), Some(hw2)) = (half(ret.wrapping_sub(4)), half(ret.wrapping_sub(2)))
        && is_32bit(hw1)
        && let Some(Instr::Bl { offset }) = decode32(hw1, hw2)
    {
        let site = ret.wrapping_sub(4);
        return Some((4, Some(site.wrapping_add(4).wrapping_add(offset as u32))));
    }
    let hw = half(ret.wrapping_sub(2))?;
    if !is_32bit(hw) && matches!(decode16(hw), Some(Instr::Blx { .. })) {
        return Some((2, None));
    }
    None
}

/// La pile d'appels, reconstituée : Thumb n'a pas de chaînage de cadres, alors on cherche dans la pile les adresses de retour
/// plausibles (un mot qui suit une instruction d'appel `bl` ou `blx` du code de l'application). Le résultat est une
/// estimation : une vieille adresse de retour restée dans la pile peut s'y glisser.
pub fn backtrace(emu: &mut Emulator, max: usize) -> Value {
    let regs = emu.cpu.regs.clone();
    let code = emu.app().map(|a| (a.flash_start, a.app_end));
    let stack_end = emu.usage().map(|u| u.stack.end);
    let mut frames = vec![frame(emu, regs.pc, None, None, "pc")];
    let (Some((code_start, code_end)), Some(stack_end)) = (code, stack_end) else {
        return Value::object().with("frames", frames);
    };
    let plausible = |v: u32| v & 1 == 1 && (code_start..code_end).contains(&(v & !1));
    let mut candidates: Vec<(u32, u32, &str)> = Vec::new();
    if plausible(regs.lr) {
        candidates.push((0, regs.lr & !1, "lr"));
    }
    let start = regs.sp & !3;
    let words = (stack_end.saturating_sub(start) / 4).min(MAX_WORDS);
    if let Ok(bytes) = emu.device.memory.read_bytes(start, words * 4) {
        for (i, chunk) in bytes.chunks_exact(4).enumerate() {
            let v = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if plausible(v) {
                candidates.push((start + 4 * i as u32, v & !1, "stack"));
            }
        }
    }
    for (slot, ret, origin) in candidates {
        if frames.len() > max {
            break;
        }
        let Some((size, target)) = call_before(emu, ret) else {
            continue;
        };
        frames
            .push(frame(emu, ret.wrapping_sub(size), Some(ret), target, origin).with("slot", slot));
    }
    Value::object().with("frames", frames)
}

fn frame(
    emu: &Emulator,
    address: u32,
    ret: Option<u32>,
    target: Option<u32>,
    origin: &str,
) -> Value {
    let mut v = Value::object()
        .with("address", address)
        .with("origin", origin);
    if let Some(ret) = ret {
        v.set("return", Value::from(ret));
    }
    if let Some(target) = target {
        v.set("target", Value::from(target));
        if let Some(text) = symbol_text(emu, target) {
            v.set("target_symbol", Value::from(text));
        }
    }
    if let Some(text) = symbol_text(emu, address) {
        v.set("symbol", Value::from(text));
    }
    v
}
