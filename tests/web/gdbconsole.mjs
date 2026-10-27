// La console de commandes à la GDB (web/src/gdbConsole.ts), contre un module simulé.
//
//   node tests/web/gdbconsole.mjs

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ts = createRequire(join(root, "web", "package.json"))("typescript");
const dir = mkdtempSync(join(tmpdir(), "nwasabi-gdb-"));
for (const name of ["util", "gdbConsole"]) {
  const { outputText } = ts.transpileModule(readFileSync(join(root, "web", "src", `${name}.ts`), "utf8"), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
  writeFileSync(join(dir, `${name}.mjs`), outputText.replace(/from "\.\/(\w+)\.js"/g, 'from "./$1.mjs"'));
}
const { runCommand } = await import(pathToFileURL(join(dir, "gdbConsole.mjs")).href);

// Un module simulé : registres, quelques octets à 0x20000000, une fonction `main` à 0x90000100.
const memory = new Uint8Array(64).map((_, i) => i);
const state = { regs: { r: [1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0], sp: 0x20000000, lr: 0x90000201, pc: 0x90000100, xpsr: 0x01000000 }, breaks: [], watches: [], running: false, steps: 0 };
const backend = {
  async request(req) {
    switch (req.cmd) {
      case "cpu": return { ok: true, ...state.regs };
      case "symbols": return { ok: true, symbols: req.query === "main" ? [{ name: "main", address: 0x90000100 }] : [] };
      case "symbol": return req.address >= 0x90000100 && req.address < 0x90000200 ? { ok: true, name: "main", offset: req.address - 0x90000100 } : { ok: true };
      case "memory_read": return { ok: true, hex: Array.from(memory.slice(req.address - 0x20000000, req.address - 0x20000000 + req.length), (b) => b.toString(16).padStart(2, "0")).join("") };
      case "break": state.breaks.push(req.address); state.lastCondition = req.condition; return { ok: true };
      case "unbreak": { const n = state.breaks.length; state.breaks = state.breaks.filter((a) => a !== req.address); return { ok: true, removed: n !== state.breaks.length }; }
      case "breaks": return { ok: true, breakpoints: state.breaks };
      case "watch": state.watches.push({ id: 7, address: req.address, length: req.length, kind: req.kind, stop: req.stop }); return { ok: true, id: 7 };
      case "watch_list": return { ok: true, watches: state.watches };
      case "cpu_set": if (req.register === "pc") state.regs.pc = req.value; else if (/^r\d+$/.test(req.register)) state.regs.r[Number(req.register.slice(1))] = req.value; return { ok: true };
      case "step": state.steps++; state.regs.pc += 2; return { ok: true };
      case "disasm": return { ok: true, lines: [{ address: req.address, size: 2, hex: "4770", mnemonic: "bx", operands: "lr" }, { address: req.address + 2, size: 2, hex: "bf00", mnemonic: "nop", operands: "" }] };
      case "backtrace": return { ok: true, frames: [{ address: 0x90000104, symbol: "main+0x4" }, { address: 0x90000300 }] };
      default: return { ok: false, error: `commande ${req.cmd}` };
    }
  },
  isRunning: () => state.running, loaded: () => true,
  async resume() { state.running = false; }, pause() {}, refresh() {},
};
const run = (line) => runCommand(line, backend);

assert.match((await run("help"))[0], /Commandes/);
assert.deepEqual(await run("break main"), ["Point d'arrêt à 0x90000100 <main>"]);
assert.deepEqual(await run("b 0x90000110"), ["Point d'arrêt à 0x90000110 <main+16>"]);
assert.deepEqual(await run("break main+8 if r0 == 5"), ["Point d'arrêt à 0x90000108 <main+8> si r0 == 5"]);
assert.deepEqual(state.lastCondition, { register: "r0", op: "==", value: 5 });
assert.match((await run("break main if r0 ~ 5"))[0], /condition/);
state.breaks.pop();
assert.equal((await run("info breakpoints")).length, 2);
assert.deepEqual(await run("delete main"), ["Point d'arrêt à 0x90000100 retiré."]);
assert.deepEqual(await run("delete 0x1"), ["Pas de point d'arrêt à 0x00000001."]);
assert.deepEqual(await run("delete"), ["1 point(s) d'arrêt retiré(s)."]);
assert.deepEqual(await run("x/4xw $sp"), ["0x20000000:\t0x03020100\t0x07060504\t0x0b0a0908\t0x0f0e0d0c"]);
assert.deepEqual(await run("x/3xb 0x20000001"), ["0x20000001:\t0x01\t0x02\t0x03"]);
assert.deepEqual(await run("x/2dh 0x20000000"), ["0x20000000:\t256\t770"]);
assert.deepEqual(await run("x/2i main"), ["0x90000100:\tbx lr", "0x90000102:\tnop"]);
assert.deepEqual(await run("p $r0+4"), ["5 (0x5)"]);
assert.deepEqual(await run("p $pc"), [`${0x90000100} (0x90000100) <main>`]);
assert.deepEqual(await run("set $r1 = 0x10"), ["r1 = 0x00000010"]);
assert.equal(state.regs.r[1], 16);
assert.deepEqual((await run("info registers r1")), ["r1   0x00000010\t16"]);
assert.equal((await run("info registers")).length, 17);
assert.deepEqual(await run("bt"), ["#0  0x90000104 in main+0x4", "#1  0x90000300"]);
assert.deepEqual(await run("si"), ["0x90000102 <main+2>"]);
assert.equal(state.steps, 1);
assert.deepEqual(await run("stepi 3"), ["0x90000108 <main+8>"]);
assert.match((await run("watch 0x20000010 8"))[0], /Point d'observation 7/);
assert.deepEqual(await run("info watchpoints"), ["7\twrite\t0x20000010 +8\tinterrompt"]);
assert.match((await run("disassemble main 2"))[0], /^ {3}0x90000100:\tbx lr$/);
assert.match((await run("frobnicate"))[0], /commande inconnue/);
assert.match((await run("p $nope"))[0], /registre inconnu/);
assert.match((await run("break unknown_fn"))[0], /symbole inconnu/);
assert.match((await run("x/4x"))[0], /adresse attendu/);
assert.match((await run("info line"))[0], /Non disponible/);
assert.deepEqual(await run(""), []);
state.running = true;
assert.match((await run("stepi"))[0], /tapez « pause »/);
console.log("ok   console GDB : points, mémoire, registres, pas-à-pas, erreurs");
