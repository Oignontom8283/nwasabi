//! Aides au débogage : noms de fonctions et pile d'appels.

use crate::cpu::{Instr, decode16, decode32, is_32bit};
use crate::emulator::{Emulator, RunStop};
use crate::json::{Value, ValueExt};

/// Nombre maximal de mots de pile examinés.
const MAX_WORDS: u32 = 8192;

/// La description d'un arrêt, avec l'adresse de l'instruction fautive (`pc`) pour une faute.
pub fn stop_json(emu: &Emulator, stop: &RunStop) -> Value {
    let mut v = super::views::stop(stop);
    if matches!(stop, RunStop::Fault(_)) && v.get("pc").is_none() {
        v.set("pc", Value::from(emu.last_fault_pc));
    }
    v
}

/// « fonction+0x10 » pour une adresse du code, ou `None`.
pub fn symbol_text(emu: &Emulator, address: u32) -> Option<String> {
    let (name, _, offset) = emu.symbol_at(address)?;
    let name = crate::symbols::demangle(name);
    Some(if offset == 0 {
        name
    } else {
        format!("{name}+{offset:#x}")
    })
}

/// Le profil d'exécution, regroupé par fonction (les adresses sans nom sont regroupées par bloc de 256 octets) et par
/// adresse : les plus fréquentes d'abord.
pub fn profile_report(emu: &Emulator, samples: &[(u32, u64)], top: usize) -> Value {
    use std::collections::HashMap;
    let total: u64 = samples.iter().map(|(_, n)| n).sum();
    let mut by_function: HashMap<(String, u32), u64> = HashMap::new();
    for &(address, count) in samples {
        let key = match emu.symbol_at(address) {
            Some((name, start, _)) => (name.to_owned(), start),
            None => (String::new(), address & !0xff),
        };
        *by_function.entry(key).or_insert(0) += count;
    }
    let mut functions: Vec<_> = by_function.into_iter().collect();
    functions.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.1.cmp(&b.0.1)));
    let mut addresses = samples.to_vec();
    addresses.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Value::object()
        .with("total", total)
        .with(
            "functions",
            functions
                .iter()
                .take(top)
                .map(|((name, start), n)| {
                    Value::object()
                        .with("name", name.as_str())
                        .with("start", *start)
                        .with("samples", *n)
                })
                .collect::<Vec<_>>(),
        )
        .with(
            "addresses",
            addresses
                .iter()
                .take(top)
                .map(|&(a, n)| {
                    let mut row = Value::object().with("address", a).with("samples", n);
                    if let Some(text) = symbol_text(emu, a) {
                        row.set("symbol", Value::from(text));
                    }
                    row
                })
                .collect::<Vec<_>>(),
        )
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
