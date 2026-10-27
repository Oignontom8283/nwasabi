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
  "  info symbol <adresse>             nom de la fonction            info line / list         (non disponibles)",
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
    if (value === undefined && /^[A-Za-z_][\w.]*$/.test(base)) {
      const found = (await backend.request({ cmd: "symbols", query: base })).symbols as Array<{ name: string; address: number }> | undefined;
      value = found?.find((s) => s.name === base)?.address;
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
      if (["line", "source", "locals", "args", "frame"].includes(sub)) return ["Non disponible : nwasabi ne lit pas les informations de débogage du code source (voir l'aide du débogage)."];
      throw new Error(`info ${sub || "?"} : sous-commande inconnue`);
    }
    case "continue": case "c": case "cont": case "run": case "r":
      await backend.resume();
      return ["Reprise."];
    case "pause": case "interrupt":
      backend.pause();
      await new Promise((r) => setTimeout(r, 60));
      return [`Arrêté à ${await stopped(backend)}`];
    case "stepi": case "si": case "step": case "s": case "nexti": case "ni": case "next": case "n": {
      paused();
      const count = rest ? Math.max(1, Number(rest) || 1) : 1;
      const over = ["nexti", "ni", "next", "n"].includes(word);
      for (let i = 0; i < count; i++) {
        if (over) {
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
    case "finish": {
      paused();
      const lr = (await registers(backend)).lr ?? 0;
      const target = (lr & ~1) >>> 0;
      await backend.request({ cmd: "break", address: target });
      await backend.resume();
      for (let wait = 0; wait < 100 && backend.isRunning(); wait++) await new Promise((r) => setTimeout(r, 30));
      await backend.request({ cmd: "unbreak", address: target });
      return [`Exécuté jusqu'au retour : ${await stopped(backend)}`];
    }
    case "print": case "p": case "inspect": case "output": {
      need("expression");
      const value = await evaluate(rest, backend);
      return [`${value >>> 0} (${hex0x(value, 1)})${await whereIs(value, backend)}`];
    }
    case "set": {
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
      const frames = ((await backend.request({ cmd: "backtrace" })).frames as Array<{ address: number; symbol?: string }>) ?? [];
      return frames.map((f, i) => `#${i}  ${hex0x(f.address, 8)}${f.symbol ? ` in ${f.symbol}` : ""}`);
    }
    default:
      throw new Error(`commande inconnue : ${word} (tapez « help »)`);
  }
}
