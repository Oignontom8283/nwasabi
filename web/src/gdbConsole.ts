// Console de commandes à la GDB : on tape `break main`, `x/8x $sp`, `bt`, `step`… et la console les traduit en requêtes au module.
// Le sous-ensemble couvert est celui qui sert au quotidien ; la fonction ne dépend que d'un `Backend`, ce qui la rend testable.

import { hex, parseAddress } from "./util.js";

/** Ce dont la console a besoin pour agir sur l'émulation. */
export interface Backend {
  request(request: Record<string, unknown>): Promise<Record<string, any> & { ok: boolean }>;
  isRunning(): boolean;
  loaded(): boolean;
  resume(): Promise<void>;
  pause(): void;
  /** Dit à l'interface que l'état a changé (rafraîchir les vues). */
  refresh(): void;
  /** Le texte d'un fichier source (par chemin DWARF), s'il a été donné. Sans lui, `list` ne montre rien. */
  sourceText?(path: string): string[] | null;
  /** Le cadre de la pile choisi (`frame`, `up`, `down`) : partagé avec la vue Source. */
  frame?: { get(): number; set(index: number): void };
}

const REGISTERS = ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "sp", "lr", "pc", "xpsr"];
const ALIASES: Record<string, string> = { fp: "r11", ip: "r12", cpsr: "xpsr", psr: "xpsr" };

const hex0x = (n: number, width = 1) => `0x${hex(n >>> 0, width)}`;

export const HELP = [
  "Commandes (abréviations entre parenthèses) :",
  "  break (b) <adresse|fonction> [if r0 == 5]   poser un point d'arrêt (conditionnel)   info breakpoints   les lister",
  "  delete (d) [adresse|fonction]     retirer un point d'arrêt (tous sans argument)",
  "  watch <adresse> [octets]          observer en écriture          rwatch / awatch          en lecture / les deux",
  "  info watchpoints                  lister les points d'observation       unwatch <n°>     en retirer un",
  "  continue (c)                      reprendre                     pause                    interrompre",
  "  stepi (si)                        une instruction               nexti (ni)               idem, sans entrer dans les appels",
  "  finish                            jusqu'au retour de la fonction",
  "  info registers (i r) [reg]        registres                     print (p) $reg           un registre",
  "  set $reg = valeur                 modifier un registre",
  "  x/NFU <adresse>                   lire la mémoire : N éléments, format x d u o t c a s i, taille b h w g",
  "  disassemble [adresse] [n]         désassembler                  backtrace (bt)           pile d'appels",
  "  info symbol <adresse>             nom de la fonction",
  "Avec les informations de débogage du code source (application compilée avec -g) :",
  "  break <fichier>:<ligne> | <ligne> | <fonction>   point d'arrêt sur une ligne ou après le prologue d'une fonction",
  "  list [<ligne>|<fonction>|<fichier>:<ligne>]       montrer le code source (les fichiers sont donnés dans la vue Source)",
  "  step (s) / next (n) / finish                      avancer d'une ligne, en entrant / sans entrer dans les appels / jusqu'au retour",
  "  print (p) <expression>            valeur d'une variable ou d'une expression C (a.b, p->x, t[2], *p, &x, n + 1)",
  "  ptype <expression|type>           décrire un type            whatis <expression>     son type seul",
  "  set var <expression> = <valeur>   modifier une variable      info locals / args / frame / source / line / variables",
  "  frame [n] / up / down             choisir un cadre de la pile (les variables et print en dépendent)    backtrace (bt) avec fonctions et lignes",
  "  watch <variable|expression>       observer l'adresse d'une variable (en écriture)",
  "Une adresse peut être un nombre (0x20000000), un registre ($sp, $pc…), un nom de fonction, avec +N ou -N.",
];

/** Évalue une expression d'adresse : nombre, `$registre`, fonction, éventuellement suivie de `+N` ou `-N`. */
export async function evaluate(expression: string, backend: Backend): Promise<number> {
  const text = expression.trim().replace(/^\*/, "");
  const match = /^(.*?)([+-]\s*(?:0x[0-9a-f]+|\d+))?$/i.exec(text);
  const base = (match?.[1] ?? text).trim();
  const offset = match?.[2] ? Number(match[2].replace(/\s+/g, "")) : 0;
  let value: number | undefined;
  if (base.startsWith("$")) {
    const name = ALIASES[base.slice(1).toLowerCase()] ?? base.slice(1).toLowerCase();
    if (!REGISTERS.includes(name)) throw new Error(`registre inconnu : ${base}`);
    value = (await registers(backend))[name];
  } else {
    value = parseAddress(base);
    if (value === undefined && /^[A-Za-z_][\w.:]*$/.test(base)) {
      const found = (await backend.request({ cmd: "symbols", query: base })).symbols as Array<{ name: string; address: number; demangled?: string; qualified?: string }> | undefined;
      // Le nom du symbole, son nom lisible (C++ et Rust) ou la fin de son nom complet (`generate`, `Chunk::generate`).
      const same = (s: { name: string; demangled?: string; qualified?: string }) => s.name === base || s.demangled === base || s.qualified === base;
      value = (found?.find(same) ?? found?.find((s) => s.qualified?.endsWith(`::${base}`)))?.address;
      if (value === undefined) throw new Error(`symbole inconnu : ${base}`);
    }
  }
  if (value === undefined) throw new Error(`expression invalide : ${expression}`);
  return (value + offset) >>> 0;
}

async function registers(backend: Backend): Promise<Record<string, number>> {
  const cpu = await backend.request({ cmd: "cpu" });
  if (!cpu.ok) throw new Error("aucune application");
  const out: Record<string, number> = { sp: cpu.sp, lr: cpu.lr, pc: cpu.pc, xpsr: cpu.xpsr };
  (cpu.r as number[]).forEach((v, i) => (out[`r${i}`] = v));
  return out;
}

async function whereIs(address: number, backend: Backend): Promise<string> {
  const sym = await backend.request({ cmd: "symbol", address });
  return sym.ok && sym.name ? ` <${sym.name}${sym.offset ? `+${sym.offset}` : ""}>` : "";
}

const hexBytes = async (backend: Backend, address: number, length: number): Promise<Uint8Array> => {
  const reply = await backend.request({ cmd: "memory_read", address, length });
  if (!reply.ok) throw new Error(String(reply.error));
  const text = String(reply.hex);
  return Uint8Array.from({ length: text.length / 2 }, (_, i) => parseInt(text.slice(2 * i, 2 * i + 2), 16));
};

const SIZES: Record<string, number> = { b: 1, h: 2, w: 4, g: 8 };

/** `x/NFU` : lit et montre la mémoire. */
async function examine(spec: string, target: string, backend: Backend): Promise<string[]> {
  const m = /^(\d*)([xduotcasi]?)([bhwg]?)$/i.exec(spec);
  if (!m) throw new Error(`format invalide : /${spec}`);
  const count = m[1] ? Number(m[1]) : 1;
  const format = (m[2] || "x").toLowerCase();
  const address = await evaluate(target, backend);
  if (format === "i") {
    const reply = await backend.request({ cmd: "disasm", address, count: Math.min(count, 256) });
    if (!reply.ok) throw new Error(String(reply.error));
    return (reply.lines as Array<{ address: number; mnemonic: string; operands: string }>).map((l) => `${hex0x(l.address, 8)}:\t${l.mnemonic} ${l.operands}`.trimEnd());
  }
  if (format === "s") {
    const bytes = await hexBytes(backend, address, Math.min(count * 64, 4096));
    const lines: string[] = [];
    let at = 0;
    for (let n = 0; n < count && at < bytes.length; n++) {
      let end = at;
      while (end < bytes.length && bytes[end] !== 0) end++;
      lines.push(`${hex0x(address + at, 8)}:\t"${String.fromCharCode(...bytes.slice(at, end)).replace(/[^\x20-\x7e]/g, (c) => `\\x${hex(c.charCodeAt(0), 2)}`)}"`);
      at = end + 1;
    }
    return lines;
  }
  const size = format === "c" ? 1 : format === "a" ? 4 : m[3] ? (SIZES[m[3].toLowerCase()] as number) : 4;
  const bytes = await hexBytes(backend, address, size * count);
  const view = new DataView(bytes.buffer);
  const perLine = size === 1 ? 8 : size === 2 ? 8 : size === 4 ? 4 : 2;
  const lines: string[] = [];
  for (let i = 0; i < count; i += perLine) {
    const cells: string[] = [];
    for (let j = i; j < Math.min(count, i + perLine); j++) {
      const raw = size === 8 ? view.getBigUint64(j * 8, true) : BigInt(size === 4 ? view.getUint32(j * 4, true) : size === 2 ? view.getUint16(j * 2, true) : view.getUint8(j));
      const bits = BigInt(size * 8);
      const signed = raw >= 1n << (bits - 1n) ? raw - (1n << bits) : raw;
      cells.push(
        format === "x" ? `0x${raw.toString(16).padStart(size * 2, "0")}`
        : format === "d" ? String(signed)
        : format === "u" ? String(raw)
        : format === "o" ? `0${raw.toString(8)}`
        : format === "t" ? raw.toString(2).padStart(size * 8, "0")
        : format === "c" ? `${Number(raw)} '${raw >= 32n && raw < 127n ? String.fromCharCode(Number(raw)) : "."}'`
        : `0x${raw.toString(16).padStart(8, "0")}${await whereIs(Number(raw), backend)}`,
      );
    }
    lines.push(`${hex0x(address + i * size, 8)}:\t${cells.join("\t")}`);
  }
  return lines;
}

async function stopped(backend: Backend, before?: { pc: number }): Promise<string> {
  const cpu = await backend.request({ cmd: "cpu" });
  backend.refresh();
  if (!cpu.ok) return "";
  void before;
  return `${hex0x(cpu.pc, 8)}${await whereIs(cpu.pc, backend)}`;
}

/** Exécute une ligne de commande ; rend les lignes à afficher. Les erreurs sont des lignes d'erreur, pas des exceptions. */
export async function runCommand(line: string, backend: Backend): Promise<string[]> {
  const text = line.trim();
  if (!text) return [];
  try {
    return await dispatch(text, backend);
  } catch (error) {
    return [`Erreur : ${(error as Error).message}`];
  }
}

// ---- code source ---------------------------------------------------------------------------------------

/** Les informations de débogage du code source de l'application, relues à chaque commande (c'est peu de chose). */
async function sourceInfo(backend: Backend): Promise<{ files: Array<{ id: number; path: string }> } | null> {
  const info = await backend.request({ cmd: "debug_info" });
  return info.ok && info.available ? (info as unknown as { files: Array<{ id: number; path: string }> }) : null;
}

const frameIndex = (backend: Backend) => backend.frame?.get() ?? 0;
let printCount = 0;

interface SourceFrame {
  index: number;
  pc: number;
  function?: string;
  symbol?: string;
  path?: string;
  line?: number;
  estimated?: boolean;
}

async function framesOf(backend: Backend, max = 64): Promise<SourceFrame[]> {
  const reply = await backend.request({ cmd: "frames", max });
  if (!reply.ok) throw new Error(String(reply.error));
  return reply.frames as SourceFrame[];
}

const shortPath = (path: string | undefined) => (path ?? "").split("/").slice(-2).join("/");

/** `fonction (a=1, b=2) at fichier:ligne`, comme GDB. */
async function describeFrame(backend: Backend, f: SourceFrame, withArgs: boolean): Promise<string> {
  const name = f.function ?? f.symbol?.replace(/\+0x[0-9a-f]+$/, "") ?? "??";
  let args = "";
  if (withArgs && f.function) {
    const locals = await backend.request({ cmd: "locals", frame: f.index });
    if (locals.ok) args = (locals.params as Array<{ name: string; value: string }>).map((p) => `${p.name}=${p.value}`).join(", ");
  }
  const where = f.line ? ` at ${shortPath(f.path)}:${f.line}` : "";
  return `${name} (${args})${where}`;
}

/** La ligne de source `n`, avec le marqueur de la ligne en cours. */
function sourceListing(lines: string[], from: number, to: number, here?: number): string[] {
  const out: string[] = [];
  for (let n = Math.max(1, from); n <= Math.min(lines.length, to); n++) out.push(`${n === here ? "=>" : "  "} ${String(n).padStart(4)}\t${lines[n - 1] ?? ""}`);
  return out;
}

async function currentLocation(backend: Backend): Promise<SourceFrame | null> {
  const frames = await framesOf(backend, frameIndex(backend) + 1).catch(() => []);
  return frames[frameIndex(backend)] ?? frames[0] ?? null;
}

/** Traduit une cible de `break` / `list` : `fichier:ligne`, `ligne`, ou `fonction`. */
async function lineTarget(text: string, backend: Backend, info: { files: Array<{ id: number; path: string }> }): Promise<{ file: number; line: number } | { name: string } | null> {
  const m = /^(?:(.+?):)?(\d+)$/.exec(text.trim());
  if (m) {
    const line = Number(m[2]);
    if (m[1]) {
      const wanted = m[1].replace(/\\/g, "/");
      const file = info.files.find((f) => f.path === wanted || f.path.endsWith(`/${wanted}`)) ?? info.files.find((f) => f.path.split("/").pop() === wanted.split("/").pop());
      if (!file) throw new Error(`fichier inconnu : ${m[1]}`);
      return { file: file.id, line };
    }
    const here = await currentLocation(backend);
    const path = here?.path ?? info.files[0]?.path;
    const file = info.files.find((f) => f.path === path) ?? info.files[0];
    return file ? { file: file.id, line } : null;
  }
  if (/^[A-Za-z_]\w*$/.test(text.trim())) return { name: text.trim() };
  return null;
}

/** Un pas d'instruction (`stepi`) ou par-dessus les appels (`nexti`), `count` fois. */
async function instructionStep(kind: "stepi" | "nexti", rest: string, backend: Backend): Promise<string[]> {
  const count = rest ? Math.max(1, Number(rest) || 1) : 1;
  for (let i = 0; i < count; i++) {
    if (kind === "nexti") {
      const here = ((await backend.request({ cmd: "disasm", address: (await registers(backend)).pc, count: 1 })).lines as Array<{ address: number; size: number; mnemonic: string }>)[0];
      if (here && /^blx?$/.test(here.mnemonic)) {
        const target = here.address + here.size;
        await backend.request({ cmd: "break", address: target });
        await backend.resume();
        for (let wait = 0; wait < 100 && backend.isRunning(); wait++) await new Promise((r) => setTimeout(r, 30));
        await backend.request({ cmd: "unbreak", address: target });
        continue;
      }
    }
    const r = await backend.request({ cmd: "step" });
    if (!r.ok) throw new Error(String(r.error));
  }
  return [await stopped(backend)];
}

/** Après un pas à la ligne : le texte d'arrivée, suivi de la ligne de source si le fichier est connu. */
async function withSourceLine(backend: Backend, arrival: string): Promise<string[]> {
  const here = await currentLocation(backend);
  const text = here?.path && here.line ? backend.sourceText?.(here.path)?.[here.line - 1] : undefined;
  return text === undefined || !here?.line ? [arrival] : [arrival, `${here.line}\t${text}`];
}

async function dispatch(text: string, backend: Backend): Promise<string[]> {
  const m = /^(\S+?)(?:\/(\S+))?(?:\s+(.*))?$/.exec(text) as RegExpExecArray;
  const word = (m[1] ?? "").toLowerCase();
  const slash = m[2] ?? "";
  const rest = (m[3] ?? "").trim();
  if (word === "help" || word === "h" || word === "?") return HELP;
  if (!backend.loaded()) return ["Aucune application chargée."];
  const need = (what: string) => {
    if (!rest) throw new Error(`${what} attendu`);
  };
  const paused = () => {
    if (backend.isRunning()) throw new Error("l'application tourne : tapez « pause » d'abord");
  };

  switch (word) {
    case "break": case "b": case "br": case "tbreak": {
      need("adresse ou fonction");
      // Avec les informations de débogage : une ligne (`fichier:ligne`, `ligne`) ou une fonction (après son prologue).
      const src = await sourceInfo(backend);
      if (src && !/\s+if\s+/.test(rest)) {
        const target = await lineTarget(rest, backend, src);
        if (target && "line" in target) {
          const r = await backend.request({ cmd: "break_line", file_id: target.file, line: target.line });
          if (!r.ok) throw new Error(String(r.error));
          backend.refresh();
          const path = src.files.find((f) => f.id === target.file)?.path;
          return [`Point d'arrêt à ${shortPath(path)}:${r.line} (${(r.addresses as number[]).map((a) => hex0x(a, 8)).join(", ")})`];
        }
        if (target && "name" in target) {
          const r = await backend.request({ cmd: "break_function", name: target.name });
          if (r.ok) {
            backend.refresh();
            return [`Point d'arrêt à ${hex0x(r.address, 8)}${r.line ? `, ${shortPath(r.path)}:${r.line}` : ""} (${target.name})`];
          }
        }
      }
      // `break <adresse> if <registre> <comparaison> <valeur>`
      const [target = "", condition = ""] = rest.split(/\s+if\s+/);
      const address = await evaluate(target, backend);
      let when: { register: string; op: string; value: number } | null = null;
      if (condition) {
        const c = /^\s*\$?(r\d{1,2}|sp|lr|pc)\s*(==|!=|<=|>=|<|>|&)\s*(.+?)\s*$/i.exec(condition);
        if (!c) throw new Error("condition : « <registre> <== != < <= > >= &> <valeur> » attendu");
        when = { register: (c[1] as string).toLowerCase(), op: c[2] as string, value: await evaluate(c[3] as string, backend) };
      }
      const reply = await backend.request({ cmd: "break", address, condition: when });
      if (!reply.ok) throw new Error(String(reply.error));
      backend.refresh();
      return [`Point d'arrêt à ${hex0x(address, 8)}${await whereIs(address, backend)}${when ? ` si ${when.register} ${when.op} ${when.value}` : ""}`];
    }
    case "delete": case "d": case "del": {
      if (!rest || rest === "breakpoints") {
        const list = ((await backend.request({ cmd: "breaks" })).breakpoints as number[]) ?? [];
        for (const address of list) await backend.request({ cmd: "unbreak", address });
        backend.refresh();
        return [`${list.length} point(s) d'arrêt retiré(s).`];
      }
      const address = await evaluate(rest, backend);
      const removed = (await backend.request({ cmd: "unbreak", address })).removed;
      backend.refresh();
      return [removed ? `Point d'arrêt à ${hex0x(address, 8)} retiré.` : `Pas de point d'arrêt à ${hex0x(address, 8)}.`];
    }
    case "watch": case "rwatch": case "awatch": {
      need("adresse");
      // Une variable ou une expression du programme : on observe l'endroit où elle se trouve.
      if (!/^(0x[0-9a-f]+|\d+|\$\w+)(\s+\d+)?$/i.test(rest) && (await sourceInfo(backend))) {
        const v = await backend.request({ cmd: "eval", expr: rest, frame: frameIndex(backend) });
        if (v.ok && typeof v.address === "number") {
          const kind = word === "watch" ? "write" : word === "rwatch" ? "read" : "readwrite";
          const reply = await backend.request({ cmd: "watch", address: v.address, length: Math.max(1, v.size ?? 4), kind, stop: true });
          if (!reply.ok) throw new Error(String(reply.error));
          backend.refresh();
          return [`Point d'observation ${reply.id} : ${rest} (${hex0x(v.address, 8)}, ${v.size} octets, ${kind}, interrompt l'application)`];
        }
        if (!v.ok) throw new Error(String(v.error));
      }
      const [target, length] = rest.split(/\s+/);
      const address = await evaluate(target ?? "", backend);
      const kind = word === "watch" ? "write" : word === "rwatch" ? "read" : "readwrite";
      const reply = await backend.request({ cmd: "watch", address, length: length ? Number(length) : 4, kind, stop: true });
      if (!reply.ok) throw new Error(String(reply.error));
      backend.refresh();
      return [`Point d'observation ${reply.id} : ${hex0x(address, 8)} (${kind}, interrompt l'application)`];
    }
    case "unwatch": {
      need("numéro");
      await backend.request({ cmd: "unwatch", id: Number(rest) });
      backend.refresh();
      return [`Point d'observation ${rest} retiré.`];
    }
    case "info": case "i": {
      const [sub = "", arg = ""] = rest.split(/\s+/);
      if (["breakpoints", "break", "b"].includes(sub)) {
        const list = ((await backend.request({ cmd: "breaks" })).breakpoints as number[]) ?? [];
        return list.length ? await Promise.all(list.map(async (a, i) => `${i + 1}\tpoint d'arrêt\t${hex0x(a, 8)}${await whereIs(a, backend)}`)) : ["Aucun point d'arrêt."];
      }
      if (["watchpoints", "watch"].includes(sub)) {
        const list = ((await backend.request({ cmd: "watch_list" })).watches as Array<{ id: number; address: number; length: number; kind: string; stop: boolean }>) ?? [];
        return list.length ? list.map((w) => `${w.id}\t${w.kind}\t${hex0x(w.address, 8)} +${w.length}${w.stop ? "\tinterrompt" : ""}`) : ["Aucun point d'observation."];
      }
      if (["registers", "reg", "r", "all-registers"].includes(sub)) {
        const all = await registers(backend);
        const names = arg ? [ALIASES[arg.replace(/^\$/, "")] ?? arg.replace(/^\$/, "")] : REGISTERS;
        return names.map((n) => {
          if (!(n in all)) throw new Error(`registre inconnu : ${n}`);
          const v = all[n] as number;
          return `${n.padEnd(5)}${hex0x(v, 8)}\t${v | 0}`;
        });
      }
      if (sub === "symbol") {
        need("adresse");
        const address = await evaluate(rest.slice(sub.length), backend);
        const where = await whereIs(address, backend);
        return [where ? `${where.trim().slice(1, -1)}` : "Aucun symbole à cette adresse."];
      }
      if (["line", "source", "locals", "args", "frame", "variables", "globals"].includes(sub)) {
        const src = await sourceInfo(backend);
        if (!src) return ["Cette application n'a pas d'informations de débogage du code source (compilez avec -g)."];
        const here = await currentLocation(backend);
        if (sub === "source") {
          return [`Fichier courant : ${here?.path ?? "(aucun)"}`, ...src.files.map((f) => `  ${f.path}`)];
        }
        if (sub === "line") {
          const target = arg ? await lineTarget(arg, backend, src) : here?.line ? { file: src.files.find((f) => f.path === here.path)?.id ?? 0, line: here.line } : null;
          if (!target || !("line" in target)) throw new Error("ligne attendue (info line 12 ou fichier:12)");
          const r = await backend.request({ cmd: "break_line", file_id: target.file, line: target.line });
          if (!r.ok) throw new Error(String(r.error));
          for (const a of r.addresses as number[]) await backend.request({ cmd: "unbreak", address: a });
          return [`La ligne ${r.line} commence à ${(r.addresses as number[]).map((a) => hex0x(a, 8)).join(", ")}.`];
        }
        if (sub === "frame") {
          const frames = await framesOf(backend);
          const f = frames[frameIndex(backend)];
          if (!f) return ["Pas de cadre."];
          return [`Cadre ${f.index} : ${await describeFrame(backend, f, true)}`, `  pc = ${hex0x(f.pc, 8)}${f.estimated ? " (estimé)" : ""}`];
        }
        if (sub === "variables" || sub === "globals") {
          const r = await backend.request({ cmd: "globals", query: arg });
          return ((r.globals as Array<{ name: string; type: string; value: string }>) ?? []).map((g) => `${g.type} ${g.name} = ${g.value}`);
        }
        const r = await backend.request({ cmd: "locals", frame: frameIndex(backend) });
        if (!r.ok) throw new Error(String(r.error));
        const list = (sub === "args" ? r.params : r.locals) as Array<{ name: string; value: string }>;
        return list.length ? list.map((v) => `${v.name} = ${v.value}`) : [sub === "args" ? "Pas d'arguments." : "Pas de variables locales."];
      }
      throw new Error(`info ${sub || "?"} : sous-commande inconnue`);
    }
    case "continue": case "c": case "cont": case "run": case "r":
      await backend.resume();
      return ["Reprise."];
    case "pause": case "interrupt":
      backend.pause();
      await new Promise((r) => setTimeout(r, 60));
      return [`Arrêté à ${await stopped(backend)}`];
    case "step": case "s": case "next": case "n": {
      paused();
      if (await sourceInfo(backend)) {
        const count = rest ? Math.max(1, Number(rest) || 1) : 1;
        let out = "";
        for (let i = 0; i < count; i++) {
          const r = await backend.request({ cmd: "step_source", mode: word === "step" || word === "s" ? "into" : "over" });
          if (!r.ok) throw new Error(String(r.error));
          out = r.line ? `${r.function ?? "??"} () at ${shortPath(r.path)}:${r.line}` : `${hex0x(r.pc, 8)}${await whereIs(r.pc, backend)}`;
        }
        backend.frame?.set(0);
        backend.refresh();
        return await withSourceLine(backend, out);
      }
      return await instructionStep(word === "next" || word === "n" ? "nexti" : "stepi", rest, backend);
    }
    case "stepi": case "si": case "nexti": case "ni": {
      paused();
      return await instructionStep(word === "nexti" || word === "ni" ? "nexti" : "stepi", rest, backend);
    }
    case "finish": {
      paused();
      if (await sourceInfo(backend)) {
        const r = await backend.request({ cmd: "step_source", mode: "out" });
        if (!r.ok) throw new Error(String(r.error));
        backend.frame?.set(0);
        backend.refresh();
        return await withSourceLine(backend, `Exécuté jusqu'au retour : ${r.function ?? "??"} () at ${shortPath(r.path)}:${r.line ?? "?"}`);
      }
      const lr = (await registers(backend)).lr ?? 0;
      const target = (lr & ~1) >>> 0;
      await backend.request({ cmd: "break", address: target });
      await backend.resume();
      for (let wait = 0; wait < 100 && backend.isRunning(); wait++) await new Promise((r) => setTimeout(r, 30));
      await backend.request({ cmd: "unbreak", address: target });
      return [`Exécuté jusqu'au retour : ${await stopped(backend)}`];
    }
    case "print": case "p": case "inspect": case "output": case "call": {
      need("expression");
      // Une expression du programme (variables, champs, tableaux, pointeurs) : le cœur l'évalue. Sinon, l'adresse ou le registre.
      if (await sourceInfo(backend)) {
        const r = await backend.request({ cmd: "eval", expr: rest, frame: frameIndex(backend) });
        if (r.ok) return [`$${++printCount} = ${r.value}`];
        if (!/inconnu|expression|texte inattendu|caractère/.test(String(r.error))) throw new Error(String(r.error));
      }
      const value = await evaluate(rest, backend);
      return [`${value >>> 0} (${hex0x(value, 1)})${await whereIs(value, backend)}`];
    }
    case "set": {
      const v = /^(?:var(?:iable)?\s+)?([^$=][^=]*?)\s*=\s*(-?\d+(?:\.\d+)?|0x[0-9a-f]+)$/i.exec(rest);
      if (v && (await sourceInfo(backend))) {
        const value = /^0x/i.test(v[2] as string) ? parseInt(v[2] as string, 16) : Number(v[2]);
        const r = await backend.request({ cmd: "assign", expr: v[1], value, frame: frameIndex(backend) });
        if (!r.ok) throw new Error(String(r.error));
        backend.refresh();
        return [`${v[1]} = ${value}`];
      }
      const m2 = /^(?:var\s+)?\$(\w+)\s*=\s*(.+)$/.exec(rest);
      if (!m2) throw new Error("syntaxe : set $registre = valeur");
      const name = ALIASES[(m2[1] as string).toLowerCase()] ?? (m2[1] as string).toLowerCase();
      if (!REGISTERS.includes(name)) throw new Error(`registre inconnu : $${m2[1]}`);
      paused();
      const value = await evaluate(m2[2] as string, backend);
      const r = await backend.request({ cmd: "cpu_set", register: name, value });
      if (!r.ok) throw new Error(String(r.error));
      backend.refresh();
      return [`${name} = ${hex0x(value, 8)}`];
    }
    case "x": {
      need("adresse");
      return await examine(slash, rest, backend);
    }
    case "disassemble": case "disas": case "disass": {
      const [target, count] = rest.split(/\s+/);
      const address = target ? await evaluate(target, backend) : (await registers(backend)).pc;
      const reply = await backend.request({ cmd: "disasm", address, count: count ? Number(count) : 12, before: 0 });
      if (!reply.ok) throw new Error(String(reply.error));
      const pc = (await registers(backend)).pc;
      return (reply.lines as Array<{ address: number; hex: string; mnemonic: string; operands: string; target_symbol?: string }>).map(
        (l) => `${l.address === pc ? "=>" : "  "} ${hex0x(l.address, 8)}:\t${l.mnemonic} ${l.operands}${l.target_symbol ? ` <${l.target_symbol}>` : ""}`.trimEnd(),
      );
    }
    case "backtrace": case "bt": case "where": {
      if (await sourceInfo(backend)) {
        const frames = await framesOf(backend);
        const out: string[] = [];
        for (const f of frames.slice(0, 32)) out.push(`#${f.index}  ${hex0x(f.pc, 8)} in ${await describeFrame(backend, f, true)}${f.estimated ? "  (estimé)" : ""}`);
        return out;
      }
      const frames = ((await backend.request({ cmd: "backtrace" })).frames as Array<{ address: number; symbol?: string }>) ?? [];
      return frames.map((f, i) => `#${i}  ${hex0x(f.address, 8)}${f.symbol ? ` in ${f.symbol}` : ""}`);
    }
    case "list": case "l": {
      const src = await sourceInfo(backend);
      if (!src) throw new Error("pas d'informations de débogage du code source (compilez avec -g)");
      const here = await currentLocation(backend);
      let path = here?.path;
      let center = here?.line ?? 1;
      if (rest) {
        const target = await lineTarget(rest, backend, src);
        if (target && "line" in target) {
          path = src.files.find((f) => f.id === target.file)?.path;
          center = target.line;
        } else if (target && "name" in target) {
          const r = await backend.request({ cmd: "break_function", name: target.name });
          if (!r.ok) throw new Error(`fonction inconnue : ${target.name}`);
          await backend.request({ cmd: "unbreak", address: r.address });
          path = r.path;
          center = r.line ?? 1;
        }
      }
      if (!path) throw new Error("aucun fichier courant");
      const lines = backend.sourceText?.(path);
      if (!lines) return [`Le texte de ${path} n'est pas chargé : donnez-le dans la vue Source.`];
      return [`${path} :`, ...sourceListing(lines, center - 5, center + 4, here?.path === path ? here.line : undefined)];
    }
    case "ptype": case "whatis": {
      need("expression ou type");
      const r = await backend.request({ cmd: "ptype", expr: rest, frame: frameIndex(backend) });
      if (!r.ok) throw new Error(String(r.error));
      if (word === "whatis") {
        const v = await backend.request({ cmd: "eval", expr: rest, frame: frameIndex(backend) });
        return [`type = ${v.ok ? v.type : String(r.type).split("\n")[0]}`];
      }
      return [`type = ${r.type}`];
    }
    case "frame": case "f": case "up": case "down": {
      const frames = await framesOf(backend);
      let index = frameIndex(backend);
      if (word === "up") index += rest ? Number(rest) : 1;
      else if (word === "down") index -= rest ? Number(rest) : 1;
      else if (rest) index = Number(rest);
      if (!Number.isInteger(index) || index < 0 || index >= frames.length) throw new Error(`pas de cadre ${rest || index}`);
      backend.frame?.set(index);
      backend.refresh();
      const f = frames[index] as SourceFrame;
      const line = f.path && f.line ? backend.sourceText?.(f.path)?.[f.line - 1] : undefined;
      return [`#${index}  ${hex0x(f.pc, 8)} in ${await describeFrame(backend, f, true)}`, ...(line === undefined ? [] : [`${f.line}\t${line}`])];
    }
    default:
      throw new Error(`commande inconnue : ${word} (tapez « help »)`);
  }
}
