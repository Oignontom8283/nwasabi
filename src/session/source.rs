//! Les commandes de débogage du code source : lignes, pile, variables, expressions, pas à pas par ligne.
//! Elles s'appuient sur les informations DWARF de l'application (`crate::debuginfo`).

use std::sync::Arc;

use super::{Reply, Session, debug, number, text};
use crate::debuginfo::{CallArg, CallResult, Context, Target, locate, read_value};
use crate::debuginfo::{DebugInfo, Frame, Place, evaluate};
use crate::emulator::{Emulator, RunStop};
use crate::json::{Value, ValueExt};

/// Les commandes de ce module.
pub(super) const COMMANDS: &[&str] = &[
    "debug_info",
    "line_at",
    "source_lines",
    "break_line",
    "unbreak_line",
    "break_function",
    "frames",
    "locals",
    "eval",
    "assign",
    "globals",
    "ptype",
    "step_source",
    "export_elf",
];

/// Au plus ce nombre d'instructions pour un pas à la ligne (une boucle sans fin ne bloque pas l'hôte).
const STEP_BUDGET: u64 = 20_000_000;

fn registers16(emu: &Emulator) -> [u32; 16] {
    let r = &emu.cpu.regs;
    let mut out = [0u32; 16];
    out[..13].copy_from_slice(&r.r);
    out[13] = r.sp;
    out[14] = r.lr;
    out[15] = r.pc & !1;
    out
}

fn read_memory(emu: &mut Emulator) -> impl FnMut(u32, usize) -> Option<Vec<u8>> + '_ {
    move |address, len| emu.device.memory.read_bytes(address, len as u32).ok()
}

/// Les cadres de la pile : avec les tables d'appel s'il y en a, sinon le seul cadre en cours.
fn frames_of(emu: &mut Emulator, info: &DebugInfo, max: usize) -> Vec<Frame> {
    let regs = registers16(emu);
    let mut read = read_memory(emu);
    let frames = info.unwind(&regs, max, &mut read);
    if frames.is_empty() {
        vec![Frame {
            pc: regs[15],
            regs,
            cfa: None,
        }]
    } else {
        frames
    }
}

/// L'adresse à regarder pour trouver la ligne et les variables d'un cadre : celle de l'appel, pas celle du retour.
fn lookup_pc(frame: &Frame, index: usize) -> u32 {
    if index == 0 {
        frame.pc
    } else {
        frame.pc.wrapping_sub(1)
    }
}

fn location_json(info: &DebugInfo, pc: u32) -> Value {
    let mut v = Value::object();
    if let Some(f) = info.function_at(pc) {
        v.set("function", f.display_name());
    }
    if let Some(row) = info.line_at(pc) {
        v.set("file", row.file);
        v.set(
            "path",
            info.files.get(row.file as usize).map_or("", String::as_str),
        );
        v.set("line", row.line);
        v.set("column", row.column);
    }
    v
}

fn frame_json(info: &DebugInfo, frame: &Frame, index: usize) -> Value {
    location_json(info, lookup_pc(frame, index))
        .with("index", index)
        .with("pc", frame.pc)
        .with("sp", frame.regs[13])
}

/// Au plus ce nombre d'instructions pour un appel de fonction demandé par une expression.
const CALL_BUDGET: u64 = 50_000_000;

/// L'appareil vu par les expressions : lecture, écriture (hors flash), registres, et appel de fonctions de l'application.
struct EmuTarget<'a> {
    emu: &'a mut Emulator,
    /// Une expression a modifié l'appareil : l'historique ne peut plus rejouer ce qui précède.
    dirty: bool,
}

impl Target for EmuTarget<'_> {
    fn read(&mut self, address: u32, len: usize) -> Option<Vec<u8>> {
        self.emu.device.memory.read_bytes(address, len as u32).ok()
    }

    fn write(&mut self, address: u32, data: &[u8]) -> bool {
        // La flash ne s'écrit pas depuis une expression (le noyau simulé est seul à la programmer).
        if (0x9000_0000..0xa000_0000).contains(&address) {
            return false;
        }
        self.dirty = true;
        self.emu.device.memory.write_bytes(address, data).is_ok()
    }

    fn set_register(&mut self, index: usize, value: u32) -> bool {
        let r = &mut self.emu.cpu.regs;
        self.dirty = true;
        match index {
            0..=12 => r.r[index] = value,
            13 => r.sp = value & !3,
            14 => r.lr = value,
            15 => r.pc = value & !1,
            _ => return false,
        }
        true
    }

    fn call(&mut self, function: u32, args: &[CallArg]) -> Result<CallResult, String> {
        let emu = &mut *self.emu;
        let entry = emu
            .app()
            .map(|a| a.entry & !1)
            .ok_or("aucune application")?;
        let (cpu, kernel, resume) = (emu.cpu.clone(), emu.kernel.clone(), emu.resume_point());
        let result = (|| {
            // Les arguments : entiers dans r0 à r3 puis sur la pile ; flottants dans s0… et doubles dans d0… (ABI à virgule flottante matérielle).
            let (mut next_r, mut next_s) = (0usize, 0usize);
            let (mut ints, mut stack): (Vec<(usize, u32)>, Vec<u32>) = (Vec::new(), Vec::new());
            let mut floats: Vec<(usize, u32)> = Vec::new();
            for arg in args {
                match *arg {
                    CallArg::Int(v) if next_r < 4 => {
                        ints.push((next_r, v));
                        next_r += 1;
                    }
                    CallArg::Int(v) => stack.push(v),
                    CallArg::Float(f) if next_s < 16 => {
                        floats.push((next_s, f.to_bits()));
                        next_s += 1;
                    }
                    CallArg::Double(d) if next_s < 15 => {
                        next_s += next_s % 2;
                        let bits = d.to_bits();
                        floats.push((next_s, bits as u32));
                        floats.push((next_s + 1, (bits >> 32) as u32));
                        next_s += 2;
                    }
                    _ => return Err("trop d'arguments à virgule flottante".to_owned()),
                }
            }
            let mut sp = emu.cpu.regs.sp.wrapping_sub(256) & !7;
            if !stack.is_empty() {
                sp = sp.wrapping_sub(4 * stack.len() as u32) & !7;
                for (i, a) in stack.iter().enumerate() {
                    emu.device
                        .memory
                        .write_u32(sp + 4 * i as u32, *a)
                        .map_err(|e| e.to_string())?;
                }
            }
            let regs = &mut emu.cpu.regs;
            for (i, v) in ints {
                regs.r[i] = v;
            }
            for (i, bits) in floats {
                regs.s[i] = bits;
            }
            regs.sp = sp;
            // Le retour de la fonction tombe sur une adresse où l'on a posé un point d'arrêt : le début de l'application.
            regs.lr = entry | 1;
            regs.pc = function & !1;
            let had = emu.breakpoints().iter().any(|b| b.address == entry);
            if !had {
                emu.add_breakpoint(entry);
            }
            emu.set_resume_point(None);
            let start = emu.cpu.cycles;
            let mut outcome = Err("l'appel ne se termine pas".to_owned());
            while emu.cpu.cycles - start < CALL_BUDGET {
                match emu.run(200_000) {
                    RunStop::Breakpoint { pc } if pc == entry => {
                        let r = &emu.cpu.regs;
                        outcome = Ok(CallResult {
                            r0: r.r[0],
                            r1: r.r[1],
                            s0: r.s[0],
                            s1: r.s[1],
                        });
                        break;
                    }
                    // Un autre point d'arrêt, la fin d'une image, un temps d'attente virtuel : l'appel continue.
                    RunStop::Breakpoint { .. } | RunStop::Budget | RunStop::Frame => {}
                    RunStop::Sleep { ms } => emu.kernel.advance_time(u64::from(ms) * 1000),
                    RunStop::Fault(f) => {
                        outcome = Err(format!("l'appel a provoqué une faute : {f:?}"));
                        break;
                    }
                    other => {
                        outcome = Err(format!("l'appel s'est arrêté : {other:?}"));
                        break;
                    }
                }
            }
            if !had {
                emu.remove_breakpoint(entry);
            }
            outcome
        })();
        // Le processeur et le noyau retrouvent leur état ; la mémoire garde les effets de l'appel, comme avec GDB.
        emu.cpu = cpu;
        emu.kernel = kernel;
        emu.set_resume_point(resume);
        self.dirty = true;
        result
    }
}

/// Évalue `body` avec le contexte du cadre `index` ; rend aussi si l'appareil a été modifié.
fn with_context<T>(
    emu: &mut Emulator,
    info: &DebugInfo,
    frame: &Frame,
    index: usize,
    body: impl FnOnce(&mut Context<'_>, u32) -> T,
) -> (T, bool) {
    let pc = lookup_pc(frame, index);
    let fregs = emu.cpu.regs.s;
    let mut target = EmuTarget { emu, dirty: false };
    let mut ctx = Context {
        regs: frame.regs,
        fregs,
        cfa: frame.cfa,
        frame_base: None,
        memory: &mut target,
    };
    // La base du cadre : évaluée une fois pour toutes les variables de la fonction.
    if let Some(expr) = info.function_at(pc).and_then(|f| f.frame_base.clone())
        && let Ok(place) = locate(&expr, &mut ctx)
    {
        ctx.frame_base = match place {
            Place::Address(a) => Some(a),
            Place::Register(r) => ctx.regs.get(usize::from(r)).copied(),
            _ => None,
        };
    }
    let result = body(&mut ctx, pc);
    (result, target.dirty)
}

fn variable_json(
    info: &DebugInfo,
    var: &crate::debuginfo::Variable,
    pc: u32,
    ctx: &mut Context<'_>,
) -> Value {
    let value = match (var.location.at(pc), var.const_value) {
        (Some(expr), _) => match locate(expr, ctx) {
            Ok(place) => read_value(info, var.ty, place, ctx),
            Err(e) => crate::debuginfo::Value {
                ty: var.ty,
                type_name: info.type_name(var.ty),
                kind: crate::debuginfo::ValueKind::Unavailable(e),
                place: None,
                size: 0,
            },
        },
        (None, Some(c)) => crate::debuginfo::Value {
            ty: var.ty,
            type_name: info.type_name(var.ty),
            kind: crate::debuginfo::ValueKind::Int(c),
            place: None,
            size: 4,
        },
        (None, None) => crate::debuginfo::Value {
            ty: var.ty,
            type_name: info.type_name(var.ty),
            kind: crate::debuginfo::ValueKind::OptimizedOut,
            place: Some(Place::OptimizedOut),
            size: 0,
        },
    };
    value
        .to_json()
        .with("name", var.name.as_str())
        .with("param", var.is_param)
}

impl Session {
    fn debug_of(&mut self) -> Result<(&mut Emulator, Arc<DebugInfo>), String> {
        let emu = self.emu()?;
        let info = emu.debug_info().ok_or(
            "l'application n'a pas d'informations de débogage du code source (compiler avec -g)",
        )?;
        Ok((emu, info))
    }

    pub(super) fn source_command(&mut self, cmd: &str, req: &Value) -> Reply {
        if cmd == "debug_info" {
            let emu = self.emu()?;
            return Ok(match emu.debug_info() {
                None => Value::object().with("available", false),
                Some(info) => Value::object()
                    .with("available", true)
                    .with(
                        "files",
                        info.files
                            .iter()
                            .enumerate()
                            .map(|(i, p)| Value::object().with("id", i).with("path", p.as_str()))
                            .collect::<Vec<_>>(),
                    )
                    .with("functions", info.functions.len())
                    .with("globals", info.globals.len())
                    .with("lines", info.rows().len()),
            });
        }
        if cmd == "export_elf" {
            let emu = self.emu()?;
            let app = emu.app().ok_or("aucune application installée")?;
            return Ok(Value::object()
                .with("name", app.name.as_str())
                .with("elf", crate::json::base64_encode(&app.to_elf())));
        }
        let (emu, info) = self.debug_of()?;
        let file_of = |req: &Value| -> Result<u32, String> {
            if let Some(id) = req.get("file_id").and_then(Value::as_u32) {
                return Ok(id);
            }
            let name = text(req, "file")?;
            info.find_file(name)
                .ok_or_else(|| format!("fichier inconnu : {name}"))
        };
        match cmd {
            "line_at" => {
                let address = number(req, "address")?;
                Ok(location_json(&info, address))
            }
            "source_lines" => {
                let file = file_of(req)?;
                let mut lines: Vec<u32> = info
                    .rows()
                    .iter()
                    .filter(|r| r.file == file && r.is_stmt && !r.end_sequence)
                    .map(|r| r.line)
                    .collect();
                lines.sort_unstable();
                lines.dedup();
                Ok(Value::object().with("file", file).with("lines", lines))
            }
            "break_line" | "unbreak_line" => {
                let file = file_of(req)?;
                let line = number(req, "line")?;
                let (resolved, addresses) = info.addresses_for_line(file, line);
                if addresses.is_empty() {
                    return Err(format!("pas de code à la ligne {line} ou après"));
                }
                let remove = cmd == "unbreak_line";
                for a in &addresses {
                    if remove {
                        emu.remove_breakpoint(*a);
                    } else {
                        emu.add_breakpoint(*a);
                    }
                }
                Ok(Value::object()
                    .with("line", resolved)
                    .with("addresses", addresses))
            }
            "break_function" => {
                let name = text(req, "name")?;
                let f = info
                    .function_named(name)
                    .ok_or_else(|| format!("fonction inconnue : {name}"))?;
                let address = info.after_prologue(f);
                emu.add_breakpoint(address);
                Ok(location_json(&info, address).with("address", address))
            }
            "frames" => {
                let max = req
                    .get("max")
                    .and_then(Value::as_u32)
                    .unwrap_or(32)
                    .clamp(1, 256) as usize;
                let frames = frames_of(emu, &info, max);
                let mut list: Vec<Value> = Vec::new();
                for (i, f) in frames.iter().enumerate() {
                    let mut v = frame_json(&info, f, i);
                    // Du code sans informations de débogage (une bibliothèque compilée sans `-g`) : le nom du symbole.
                    if v.get("function").is_none()
                        && let Some((name, _, offset)) = emu.symbol_at(lookup_pc(f, i))
                    {
                        v.set(
                            "symbol",
                            format!("{}+{offset:#x}", crate::symbols::demangle(name)),
                        );
                    }
                    list.push(v);
                }
                // Les tables d'appel s'arrêtent au premier code qu'elles ne décrivent pas : on complète alors avec l'estimation
                // fondée sur les adresses de retour de la pile.
                let incomplete = frames.len() <= 2
                    && frames.last().is_some_and(|f| {
                        info.function_at(lookup_pc(f, frames.len() - 1)).is_none()
                    });
                let mut estimated = false;
                if incomplete {
                    let guess = debug::backtrace(emu, max);
                    if let Some(items) = guess.get("frames").and_then(Value::as_array) {
                        let known: Vec<u32> = frames.iter().map(|f| f.pc).collect();
                        for item in items.iter().skip(1) {
                            let pc = item.get("return").and_then(Value::as_u32).unwrap_or(0);
                            if known.contains(&pc) {
                                continue;
                            }
                            let address = item.get("address").and_then(Value::as_u32).unwrap_or(pc);
                            let mut v = location_json(&info, address)
                                .with("index", list.len())
                                .with("pc", pc)
                                .with("estimated", true);
                            if let Some(sym) = item.get("symbol").and_then(Value::as_str) {
                                v.set("symbol", sym);
                            }
                            list.push(v);
                            estimated = true;
                        }
                    }
                }
                Ok(Value::object()
                    .with("frames", list)
                    .with("estimated", estimated))
            }
            "locals" | "eval" | "assign" | "ptype" | "globals" => {
                let index = req.get("frame").and_then(Value::as_u32).unwrap_or(0) as usize;
                let frames = frames_of(emu, &info, index + 1);
                let frame = *frames.get(index).ok_or("pas de tel cadre")?;
                let function = info.function_at(lookup_pc(&frame, index));
                let fb = function.and_then(|f| f.frame_base.clone());
                // Une expression peut modifier l'appareil (affectation, appel) : l'historique repart alors de là.
                let (reply, dirty): (Reply, bool) = match cmd {
                    "locals" => with_context(emu, &info, &frame, index, |ctx, pc| {
                        let vars = info.visible_locals(pc);
                        let mut params = Vec::new();
                        let mut locals = Vec::new();
                        for v in vars {
                            let j = variable_json(&info, v, pc, ctx);
                            if v.is_param {
                                params.push(j)
                            } else {
                                locals.push(j)
                            }
                        }
                        Ok(Value::object()
                            .with("params", params)
                            .with("locals", locals))
                    }),
                    "globals" => {
                        let query = req
                            .get("query")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_lowercase();
                        with_context(emu, &info, &frame, index, |ctx, pc| {
                            let list: Vec<Value> = info
                                .globals
                                .iter()
                                .filter(|g| g.name.to_lowercase().contains(&query))
                                .take(200)
                                .map(|g| variable_json(&info, g, pc, ctx))
                                .collect();
                            Ok(Value::object().with("globals", list))
                        })
                    }
                    "eval" => {
                        let expr = text(req, "expr")?;
                        with_context(emu, &info, &frame, index, |ctx, pc| {
                            evaluate(&info, expr, pc, ctx, fb.as_deref())
                                .map(|v| v.to_json())
                                .map_err(|e| e.to_string())
                        })
                    }
                    "ptype" => {
                        let expr = text(req, "expr")?;
                        // D'abord un nom de type ou de structure, puis une expression.
                        if let Some(t) = find_type(&info, expr) {
                            return Ok(Value::object().with("type", describe_type(&info, t)));
                        }
                        with_context(emu, &info, &frame, index, |ctx, pc| {
                            let v = evaluate(&info, expr, pc, ctx, fb.as_deref())
                                .map_err(|e| e.to_string())?;
                            Ok(Value::object().with(
                                "type",
                                v.ty.map_or_else(
                                    || v.type_name.clone(),
                                    |t| describe_type(&info, t),
                                ),
                            ))
                        })
                    }
                    _ => {
                        // assign : `expr = valeur`, avec la syntaxe des expressions.
                        let expr = text(req, "expr")?;
                        let new = req.get("value").ok_or("champ « value » manquant")?;
                        let literal = match (new.as_i64(), new.as_f64()) {
                            (Some(n), _) => n.to_string(),
                            (_, Some(f)) => format!("{f:?}"),
                            _ => return Err("une valeur numérique est attendue".into()),
                        };
                        let assignment = format!("({expr}) = {literal}");
                        with_context(emu, &info, &frame, index, |ctx, pc| {
                            evaluate(&info, &assignment, pc, ctx, fb.as_deref())
                                .map(|v| {
                                    let mut reply = Value::object().with("size", v.size);
                                    if let Some(Place::Address(a)) = v.place {
                                        reply.set("address", a);
                                    }
                                    reply.with("value", v.summary())
                                })
                                .map_err(|e| e.to_string())
                        })
                    }
                };
                if dirty {
                    self.history_reset();
                }
                reply
            }
            "step_source" => self.step_source(req),
            _ => Err(format!("commande inconnue : {cmd}")),
        }
    }

    /// Un pas à la ligne : `into` (entre dans les fonctions), `over` (les exécute en entier), `out` (jusqu'au retour de la fonction).
    fn step_source(&mut self, req: &Value) -> Reply {
        let mode = req
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("over")
            .to_owned();
        if !matches!(mode.as_str(), "into" | "over" | "out") {
            return Err(format!("mode inconnu : {mode} (into, over, out)"));
        }
        self.history_fork();
        let (emu, info) = self.debug_of()?;
        let start_cycles = emu.cpu.cycles;
        let start_frames = frames_of(emu, &info, 2);
        let start_pc = emu.cpu.regs.pc & !1;
        let start_line = info.line_at(start_pc).map(|r| (r.file, r.line));
        let start_cfa = start_frames.first().and_then(|f| f.cfa);
        let start_function = info.function_at(start_pc).map(|f| f.low);
        let mut stop = RunStop::Budget;
        let mut outcome = "line";

        // Rejoindre l'adresse de retour du cadre en cours, quand la pile est revenue au cadre appelant.
        let run_to_return = |emu: &mut Emulator, ret: u32, min_sp: u32| -> Option<RunStop> {
            let had = emu.breakpoints().iter().any(|b| b.address == ret);
            if !had {
                emu.add_breakpoint(ret);
            }
            let mut result = None;
            while emu.cpu.cycles - start_cycles < STEP_BUDGET {
                let s = emu.run(500_000);
                match s {
                    RunStop::Breakpoint { pc } if pc == ret && emu.cpu.regs.sp >= min_sp => break,
                    RunStop::Breakpoint { .. } | RunStop::Budget => {}
                    other => {
                        result = Some(other);
                        break;
                    }
                }
            }
            if !had {
                emu.remove_breakpoint(ret);
            }
            result
        };

        if mode == "out" {
            let Some(caller) = start_frames.get(1) else {
                return Err("pas de fonction appelante (sommet de la pile)".into());
            };
            let cfa = start_frames[0].cfa.unwrap_or(emu.cpu.regs.sp);
            if let Some(s) = run_to_return(emu, caller.pc, cfa) {
                stop = s;
                outcome = "stopped";
            }
        } else {
            let mut steps = 0u64;
            loop {
                if emu.cpu.cycles - start_cycles >= STEP_BUDGET {
                    outcome = "budget";
                    break;
                }
                let pc = emu.cpu.regs.pc & !1;
                let is_call = is_call_at(emu, pc);
                let sp_before = emu.cpu.regs.sp;
                stop = emu.step();
                steps += 1;
                match stop {
                    RunStop::Budget | RunStop::Frame => {}
                    RunStop::Sleep { ms } => {
                        emu.kernel.advance_time(u64::from(ms) * 1000);
                    }
                    RunStop::WaitEvent { .. } => {
                        outcome = "waiting";
                        break;
                    }
                    _ => {
                        outcome = "stopped";
                        break;
                    }
                }
                let now = emu.cpu.regs.pc & !1;
                // Un appel : « over » le laisse finir ; « into » y entre seulement s'il y a des lignes (sinon il le laisse finir aussi).
                if is_call && now != pc.wrapping_add(4) && now != pc.wrapping_add(2) {
                    let enters_known = info.function_at(now).is_some();
                    if mode == "over" || !enters_known {
                        let ret = emu.cpu.regs.lr & !1;
                        if let Some(s) = run_to_return(emu, ret, sp_before) {
                            stop = s;
                            outcome = "stopped";
                            break;
                        }
                    } else {
                        // Dans la fonction : on file jusqu'à la première ligne après le prologue.
                        if let Some(f) = info.function_at(now) {
                            let target = info.after_prologue(f);
                            if target != now {
                                let had = emu.breakpoints().iter().any(|b| b.address == target);
                                if !had {
                                    emu.add_breakpoint(target);
                                }
                                let s = emu.run(STEP_BUDGET);
                                if !had {
                                    emu.remove_breakpoint(target);
                                }
                                if !matches!(s, RunStop::Breakpoint { .. }) {
                                    stop = s;
                                    outcome = "stopped";
                                }
                            }
                            break;
                        }
                    }
                }
                let now = emu.cpu.regs.pc & !1;
                if let Some(row) = info.line_at(now)
                    && row.address == now
                    && row.is_stmt
                {
                    let line = (row.file, row.line);
                    let cfa = frames_of(emu, &info, 1).first().and_then(|f| f.cfa);
                    let same_function = info.function_at(now).map(|f| f.low) == start_function;
                    // Nouvelle ligne dans la même fonction, ou retour dans l'appelant (cadre différent).
                    if Some(line) != start_line || !same_function || cfa != start_cfa {
                        break;
                    }
                }
                if steps > STEP_BUDGET {
                    outcome = "budget";
                    break;
                }
            }
        }
        let pc = emu.cpu.regs.pc & !1;
        let reply = location_json(&info, pc)
            .with("pc", pc)
            .with("outcome", outcome)
            .with("stop", debug::stop_json(emu, &stop))
            .with("instructions", emu.cpu.cycles - start_cycles)
            .with("time_ms", emu.time_us() / 1000);
        self.history_tick();
        Ok(reply)
    }
}

/// L'instruction à `pc` est-elle un appel (`bl`, `blx`) ?
fn is_call_at(emu: &mut Emulator, pc: u32) -> bool {
    let memory = &mut emu.device.memory;
    let mut read = |a: u32| {
        let b = memory.read_bytes(a, 2).ok()?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    };
    crate::cpu::disassemble(&mut read, pc, 1)
        .first()
        .is_some_and(|l| l.mnemonic.starts_with("bl"))
}

fn find_type(info: &DebugInfo, name: &str) -> Option<usize> {
    let wanted = name.trim();
    let bare = wanted
        .strip_prefix("struct ")
        .or_else(|| wanted.strip_prefix("union "))
        .or_else(|| wanted.strip_prefix("enum "))
        .unwrap_or(wanted);
    (0..info.type_count()).find(|&i| {
        info.ty(i).name.as_deref() == Some(bare)
            && !matches!(info.ty(i).kind, crate::debuginfo::TypeKind::Unknown)
    })
}

/// La description d'un type, comme `ptype` : les champs d'une structure, les valeurs d'une énumération.
fn describe_type(info: &DebugInfo, id: usize) -> String {
    use crate::debuginfo::TypeKind;
    let stripped = info.strip(id);
    let t = info.ty(stripped);
    match &t.kind {
        TypeKind::Struct { fields, union, .. } => {
            let mut s = format!(
                "{} {} {{\n",
                if *union { "union" } else { "struct" },
                t.name.as_deref().unwrap_or("")
            );
            for f in fields {
                s += &format!(
                    "    {} {};  /* +{} */\n",
                    info.type_name(Some(f.ty)),
                    f.name,
                    f.offset
                );
            }
            s + &format!("}} /* {} octets */", info.size_of(stripped))
        }
        TypeKind::Enum { variants, .. } => {
            let list: Vec<String> = variants.iter().map(|(n, v)| format!("{n} = {v}")).collect();
            format!(
                "enum {} {{{}}}",
                t.name.as_deref().unwrap_or(""),
                list.join(", ")
            )
        }
        _ => info.type_name(Some(id)),
    }
}
