// Descriptions des instructions et genre des sauts (web/src/thumbDocs.ts).
//
//   node tests/web/thumbdocs.mjs

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ts = createRequire(join(root, "web", "package.json"))("typescript");
const { outputText } = ts.transpileModule(readFileSync(join(root, "web", "src", "thumbDocs.ts"), "utf8"), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
const out = join(mkdtempSync(join(tmpdir(), "nwasabi-docs-")), "thumbDocs.mjs");
writeFileSync(out, outputText);
const { describe, jumpKind, tooltip } = await import(pathToFileURL(out).href);

assert.equal(describe("movs").name, "mov");
assert.equal(describe("movs").flags, true);
assert.equal(describe("addeq").name, "add");
assert.match(describe("addeq").condition, /égal/);
assert.equal(describe("bne").name, "b");
assert.equal(describe("vadd.f32").name, "vadd");
assert.equal(describe("subs").flags, true);
assert.equal(describe("ldrb").name, "ldrb");
assert.equal(describe("ite").name, "it");
assert.equal(describe("zzz"), null);
assert.equal(describe("ldrsh").name, "ldrsh");
assert.equal(describe("bls").name, "b", "bls : branchement « inférieur ou égal non signé », pas « bl » + s");

assert.equal(jumpKind("b", "0x100", true), "jump");
assert.equal(jumpKind("bne", "0x100", true), "branch");
assert.equal(jumpKind("bl", "0x100", true), "call");
assert.equal(jumpKind("blx", "r3", false), "indirect");
assert.equal(jumpKind("bx", "lr", false), "return");
assert.equal(jumpKind("bx", "r2", false), "indirect");
assert.equal(jumpKind("pop", "{r4, pc}", false), "return");
assert.equal(jumpKind("pop", "{r4, r5}", false), null);
assert.equal(jumpKind("cbz", "r0, 0x10", true), "branch");
assert.equal(jumpKind("movs", "r0, #1", false), null);
assert.equal(jumpKind("bics", "r0, r1", false), null);
assert.match(tooltip("beq", "0x10", true), /Saut conditionnel/);
assert.match(tooltip("adds", "r0, r1, r2", false), /indicateurs/);
console.log("ok   descriptions d'instructions : conditions, suffixes, genres de saut");
