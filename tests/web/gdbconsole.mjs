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
assert.match((await run("info line"))[0], /informations de débogage/);
assert.deepEqual(await run(""), []);
state.running = true;
assert.match((await run("stepi"))[0], /tapez « pause »/);
console.log("ok   console GDB : points, mémoire, registres, pas-à-pas, erreurs");

state.running = false;
// ---- avec les informations de débogage du code source ----------------------------------------------------------
const lines = ["#include <x.h>", "int add(int a, int b) {", "  int sum = a + b;", "  return sum;", "}", "int main(void) {", "  return add(2, 3);", "}"];
const code = { frame: 0, steps: [], assigned: null, watches: [], stepped: false };
const sourceBackend = {
  ...backend,
  sourceText: (path) => (path === "src/app.c" ? lines : null),
  frame: { get: () => code.frame, set: (i) => (code.frame = i) },
  async request(req) {
    const frame = (i) => [{ index: 0, pc: 0x90000110, function: "add", path: "src/app.c", line: 3 }, { index: 1, pc: 0x90000150, function: "main", path: "src/app.c", line: 7 }][i];
    switch (req.cmd) {
      case "debug_info": return { ok: true, available: true, files: [{ id: 0, path: "src/app.c" }] };
      case "frames": return { ok: true, frames: code.stepped ? [{ ...frame(1), index: 0 }] : [frame(0), frame(1)] };
      case "locals": return req.frame === 0 ? { ok: true, params: [{ name: "a", value: "2", type: "int" }, { name: "b", value: "3", type: "int" }], locals: [{ name: "sum", value: "5", type: "int" }] } : { ok: true, params: [], locals: [] };
      case "eval":
        if (req.expr === "sum") return { ok: true, type: "int", value: "5", address: 0x2000ff00, size: 4 };
        if (req.expr === "a + b") return { ok: true, type: "int", value: "5", size: 4 };
        return { ok: false, error: `symbole inconnu : ${req.expr}` };
      case "break_line": return { ok: true, line: req.line < 2 ? 2 : req.line, addresses: [0x90000104] };
      case "break_function": return req.name === "add" ? { ok: true, address: 0x90000108, line: 3, path: "src/app.c", function: "add" } : { ok: false, error: "fonction inconnue" };
      case "step_source": code.steps.push(req.mode); code.stepped = true; return { ok: true, function: "main", path: "src/app.c", line: 7, pc: 0x90000150 };
      case "ptype": return req.expr === "Point" ? { ok: true, type: "struct Point {\n    int x;\n} /* 4 octets */" } : { ok: true, type: "int" };
      case "assign": code.assigned = req; return { ok: true };
      case "globals": return { ok: true, globals: [{ name: "counter", type: "int", value: "7" }] };
      case "watch": code.watches.push(req); return { ok: true, id: 9 };
      default: return backend.request(req);
    }
  },
};
const src = (line) => runCommand(line, sourceBackend);
assert.deepEqual(await src("break src/app.c:3"), ["Point d'arrêt à src/app.c:3 (0x90000104)"]);
assert.deepEqual(await src("b 1"), ["Point d'arrêt à src/app.c:2 (0x90000104)"], "une ligne sans code se pose sur la suivante");
assert.match((await src("break add"))[0], /Point d'arrêt à 0x90000108, src\/app.c:3 \(add\)/);
assert.match((await src("break nowhere.c:3"))[0], /fichier inconnu/);
const listing = await src("list");
assert.equal(listing[0], "src/app.c :");
assert.match(listing[3], /^=> {1}\s*3\t {2}int sum = a \+ b;$/, "la ligne en cours est marquée");
assert.deepEqual((await src("list 6")).slice(0, 2), ["src/app.c :", "      1\t#include <x.h>"]);
assert.deepEqual(await src("step"), ["main () at src/app.c:7", "7\t  return add(2, 3);"]);
assert.deepEqual(await src("next"), ["main () at src/app.c:7", "7\t  return add(2, 3);"]);
assert.deepEqual(code.steps, ["into", "over"]);
assert.match((await src("finish"))[0], /Exécuté jusqu'au retour : main \(\) at src\/app.c:7/);
assert.deepEqual(code.steps.at(-1), "out");
code.stepped = false;
assert.deepEqual(await src("print sum"), ["$1 = 5"]);
assert.deepEqual(await src("p a + b"), ["$2 = 5"]);
assert.match((await src("print nothing"))[0], /symbole inconnu/);
assert.deepEqual(await src("p $r0+4"), ["5 (0x5)"], "une expression de registre retombe sur l'évaluation d'adresse");
assert.deepEqual(await src("ptype Point"), ["type = struct Point {\n    int x;\n} /* 4 octets */"]);
assert.deepEqual(await src("whatis sum"), ["type = int"]);
assert.deepEqual(await src("info locals"), ["sum = 5"]);
assert.deepEqual(await src("info args"), ["a = 2", "b = 3"]);
assert.deepEqual(await src("info source"), ["Fichier courant : src/app.c", "  src/app.c"]);
assert.match((await src("info frame"))[0], /Cadre 0 : add \(a=2, b=3\) at src\/app.c:3/);
assert.deepEqual(await src("bt"), ["#0  0x90000110 in add (a=2, b=3) at src/app.c:3", "#1  0x90000150 in main () at src/app.c:7"]);
assert.match((await src("up"))[0], /^#1 {2}0x90000150 in main/);
assert.equal(code.frame, 1);
assert.match((await src("up"))[0], /pas de cadre 2/);
assert.match((await src("down"))[0], /^#0 {2}0x90000110 in add/);
assert.match((await src("frame 1"))[1], /^7\t/);
code.frame = 0;
assert.deepEqual(await src("set var sum = 9"), ["sum = 9"]);
assert.equal(code.assigned.expr, "sum");
assert.equal(code.assigned.value, 9);
assert.match((await src("watch sum"))[0], /Point d'observation 9 : sum \(0x2000ff00, 4 octets, write/);
assert.deepEqual(code.watches[0], { cmd: "watch", address: 0x2000ff00, length: 4, kind: "write", stop: true });
assert.deepEqual(await src("info variables"), ["int counter = 7"]);
console.log("gdbconsole : ok");
