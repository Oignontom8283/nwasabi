#!/usr/bin/env python3
"""Génère les vecteurs de test du processeur en les faisant exécuter par QEMU.

Pour chaque « lot », on écrit un petit programme Thumb-2 qui enchaîne des centaines de cas de test :
chaque cas charge des valeurs initiales dans r0-r11 et dans les indicateurs, exécute quelques
instructions, puis copie les registres, les indicateurs et un tampon mémoire dans une zone de
résultats. QEMU (machine `mps2-an500`, Cortex-M7) exécute le programme ; la zone de résultats est
sauvegardée. Le test Rust `tests/cpu_vectors.rs` rejoue ensuite les mêmes programmes sur le processeur
de nwasabi et compare les résultats octet pour octet.

Fichiers produits dans le dossier de sortie, pour chaque lot `NN_nom` :
  NN_nom.bin       programme (à charger à l'adresse 0)
  NN_nom.expected  résultats de QEMU (cas après cas, puis un mot-marqueur)
  NN_nom.cases     le texte de chaque cas, pour lire les différences

Outils nécessaires (variables d'environnement facultatives entre crochets) :
  arm-none-eabi-as / -ld / -objcopy   [ARM_AS, ARM_LD, ARM_OBJCOPY]
  qemu-system-arm                     [QEMU, QEMU_ARGS : arguments supplémentaires, ex. "-L dossier"]

Usage : python3 tests/cpu/generate_vectors.py [--out tests/golden/cpu] [--seed 1] [--only motif]
"""
import argparse
import os
import random
import shlex
import subprocess
import sys
import tempfile
import time

BUF = 0x20008000      # tampon de 64 octets, réinitialisé à chaque cas
OUT = 0x20010000      # zone des résultats
CASE_WORDS = 29       # r0-r11, APSR, 16 mots de tampon
MARKER = 0x00C0FFEE
FP_CASE_WORDS = 62     # r0-r11, APSR, FPSCR, s0-s31, 16 mots de tampon

COND = ['eq', 'ne', 'cs', 'cc', 'mi', 'pl', 'vs', 'vc', 'hi', 'ls', 'ge', 'lt', 'gt', 'le']
INVERSE = {'eq': 'ne', 'ne': 'eq', 'cs': 'cc', 'cc': 'cs', 'mi': 'pl', 'pl': 'mi', 'vs': 'vc',
           'vc': 'vs', 'hi': 'ls', 'ls': 'hi', 'ge': 'lt', 'lt': 'ge', 'gt': 'le', 'le': 'gt'}
LOW = list(range(0, 8))
ANY = list(range(0, 11))   # r11 = tampon, r12 = temporaire, sp/lr/pc exclus

SPECIAL = [0, 1, 2, 3, 0x7f, 0x80, 0xff, 0x100, 0x7fff, 0x8000, 0xffff, 0x10000, 0x7fffffff,
           0x80000000, 0x80000001, 0xffffffff, 0xfffffffe, 0x55555555, 0xaaaaaaaa, 0x12345678,
           0xdeadbeef, 0x0000ffff, 0xffff0000]


def rv():
    """Valeur initiale de registre : souvent une valeur « limite », parfois aléatoire."""
    return random.choice(SPECIAL) if random.random() < 0.6 else random.getrandbits(32)


def reg(pool=ANY):
    return 'r%d' % random.choice(pool)


def regs_distinct(n, pool=ANY):
    return ['r%d' % r for r in random.sample(pool, n)]


def immd(bits):
    return random.randrange(1 << bits)


def cond():
    return random.choice(COND)


def modified_immediate():
    """Constante « modifiée » de Thumb-2 : octet, motifs répétés, ou octet tourné."""
    form = random.randrange(5)
    b = random.randrange(256)
    if form == 0:
        return b
    if form == 1:
        return (b << 16) | b
    if form == 2:
        return (b << 24) | (b << 8)
    if form == 3:
        return (b << 24) | (b << 16) | (b << 8) | b
    byte = 0x80 | random.randrange(128)
    rot = random.randrange(8, 32)
    return ((byte >> rot) | (byte << (32 - rot))) & 0xffffffff


# ---------------------------------------------------------------------------------------------
# Catalogue des cas : chaque fonction renvoie une liste d'extraits d'assembleur
# ---------------------------------------------------------------------------------------------

def cases_thumb16():
    out = []
    for _ in range(80):
        rd, rm, rn = reg(LOW), reg(LOW), reg(LOW)
        gens = [
            lambda: f"movs.n {rd}, #{immd(8)}",
            lambda: f"cmp.n {rn}, #{immd(8)}",
            lambda: f"adds.n {rd}, #{immd(8)}",
            lambda: f"subs.n {rd}, #{immd(8)}",
            lambda: f"adds.n {rd}, {rn}, #{immd(3)}",
            lambda: f"subs.n {rd}, {rn}, #{immd(3)}",
            lambda: f"adds.n {rd}, {rn}, {rm}",
            lambda: f"subs.n {rd}, {rn}, {rm}",
            lambda: f"lsls.n {rd}, {rm}, #{random.randrange(32)}",
            lambda: f"lsrs.n {rd}, {rm}, #{random.randrange(1, 33)}",
            lambda: f"asrs.n {rd}, {rm}, #{random.randrange(1, 33)}",
            lambda: f"{random.choice(['ands', 'eors', 'adcs', 'sbcs', 'orrs', 'bics'])} {rd}, {rm}",
            lambda: f"{random.choice(['lsls', 'lsrs', 'asrs', 'rors'])} {rd}, {rm}",
            lambda: f"muls {rd}, {rm}, {rd}",
            lambda: f"{random.choice(['tst', 'cmn', 'cmp'])}.n {rn}, {rm}",
            lambda: f"mvns {rd}, {rm}",
            lambda: f"rsbs {rd}, {rm}, #0",
            lambda: f"{random.choice(['add', 'cmp', 'mov'])}.n {reg()}, {reg()}",
            lambda: f"{random.choice(['sxth', 'sxtb', 'uxth', 'uxtb', 'rev', 'rev16', 'revsh'])}.n {rd}, {rm}",
        ]
        out.append(random.choice(gens)())
    return out


def cases_it_blocks():
    out = []
    patterns = ['', 't', 'e', 'tt', 'te', 'et', 'ee', 'ttt', 'tte', 'tet', 'tee', 'ett', 'ete',
                'eet', 'eee']
    for _ in range(80):
        c = cond()
        pat = random.choice(patterns)
        if random.random() < 0.3:
            lines = [f"cmp {reg()}, #{immd(8)}"]
        else:
            lines = [f"cmp {reg()}, {reg()}"]
        lines.append(f"it{pat} {c}")
        conds = [c] + [c if ch == 't' else INVERSE[c] for ch in pat]
        for cc in conds:
            rd, rm, rn = reg(LOW), reg(LOW), reg(LOW)
            lines.append(random.choice([
                f"mov{cc} {rd}, #{immd(8)}",
                f"add{cc} {rd}, {rn}, {rm}",
                f"sub{cc} {rd}, {rn}, {rm}",
                f"add{cc} {rd}, {rn}, #{immd(3)}",
                f"and{cc} {rd}, {rm}",
                f"orr{cc} {rd}, {rm}",
                f"eor{cc} {rd}, {rm}",
                f"mvn{cc} {rd}, {rm}",
                f"rsb{cc} {rd}, {rm}, #0",
                f"lsl{cc} {rd}, {rm}, #{random.randrange(32)}",
                f"mul{cc} {rd}, {rm}, {rd}",
                f"adc{cc} {rd}, {rm}",
                f"sbc{cc} {rd}, {rm}",
                f"mov{cc}.w {reg()}, #{modified_immediate()}",
                f"add{cc}.w {reg()}, {reg()}, #{immd(8)}",
                f"ldr{cc} {rd}, [r11, #{4 * random.randrange(8)}]",
                f"str{cc} {rd}, [r11, #{4 * random.randrange(8)}]",
                f"ldrb{cc} {rd}, [r11, #{random.randrange(16)}]",
                f"uxtb{cc} {rd}, {rm}",
            ]))
        out.append('\n'.join(lines))
    return out


DP_OPS = ['and', 'bic', 'orr', 'orn', 'eor', 'add', 'adc', 'sbc', 'sub', 'rsb']


def cases_thumb32_dp():
    out = []
    for _ in range(160):
        op = random.choice(DP_OPS)
        s = random.choice(['', 's'])
        rd, rn, rm = reg(), reg(), reg()
        const = modified_immediate()
        shift = random.choice(['lsl', 'lsr', 'asr', 'ror'])
        amount = random.randrange(1, 32)
        out.append(random.choice([
            f"{op}{s}.w {rd}, {rn}, #{const}",
            f"{op}{s}.w {rd}, {rn}, {rm}",
            f"{op}{s}.w {rd}, {rn}, {rm}, {shift} #{amount}",
            f"{op}{s}.w {rd}, {rn}, {rm}, rrx",
            f"{random.choice(['tst', 'teq', 'cmn', 'cmp'])}.w {rn}, #{const}",
            f"{random.choice(['tst', 'teq', 'cmn', 'cmp'])}.w {rn}, {rm}, {shift} #{amount}",
            f"mov{s}.w {rd}, #{const}",
            f"mvn{s}.w {rd}, #{const}",
            f"mov{s}.w {rd}, {rm}",
            f"mvn{s}.w {rd}, {rm}, {shift} #{amount}",
            f"{random.choice(['lsl', 'lsr', 'asr', 'ror'])}{s}.w {rd}, {rm}, #{amount}",
            f"rrx{s} {rd}, {rm}",
            f"{random.choice(['lsl', 'lsr', 'asr', 'ror'])}{s}.w {rd}, {rn}, {rm}",
            f"addw {rd}, {rn}, #{immd(12)}",
            f"subw {rd}, {rn}, #{immd(12)}",
            f"movw {rd}, #{immd(16)}",
            f"movt {rd}, #{immd(16)}",
        ]))
    return out


def cases_bitfield():
    out = []
    for _ in range(120):
        rd, rn = reg(), reg()
        lsb = random.randrange(32)
        width = random.randrange(1, 33 - lsb)
        out.append(random.choice([
            f"ubfx {rd}, {rn}, #{lsb}, #{width}",
            f"sbfx {rd}, {rn}, #{lsb}, #{width}",
            f"bfi {rd}, {rn}, #{lsb}, #{width}",
            f"bfc {rd}, #{lsb}, #{width}",
            f"ssat {rd}, #{random.randrange(1, 33)}, {rn}",
            f"ssat {rd}, #{random.randrange(1, 33)}, {rn}, lsl #{random.randrange(32)}",
            f"ssat {rd}, #{random.randrange(1, 33)}, {rn}, asr #{random.randrange(1, 32)}",
            f"usat {rd}, #{random.randrange(0, 32)}, {rn}",
            f"usat {rd}, #{random.randrange(0, 32)}, {rn}, lsl #{random.randrange(32)}",
            f"usat {rd}, #{random.randrange(0, 32)}, {rn}, asr #{random.randrange(1, 32)}",
            f"clz {rd}, {rn}",
            f"rbit {rd}, {rn}",
            f"rev.w {rd}, {rn}",
            f"rev16.w {rd}, {rn}",
            f"revsh.w {rd}, {rn}",
        ]))
    return out


def cases_extend_parallel():
    out = []
    par_ops = ['uadd8', 'usub8', 'sadd8', 'ssub8', 'uqadd8', 'uqsub8', 'uadd16', 'usub16',
               'sadd16', 'ssub16', 'uqadd16', 'uqsub16']
    for _ in range(140):
        rd, rn, rm = reg(), reg(), reg()
        rot = random.choice(['', ', ror #8', ', ror #16', ', ror #24'])
        par = random.choice(par_ops)
        out.append(random.choice([
            f"{random.choice(['sxtb', 'sxth', 'uxtb', 'uxth', 'uxtb16', 'sxtb16'])}.w {rd}, {rm}{rot}",
            f"{random.choice(['sxtab', 'sxtah', 'uxtab', 'uxtah', 'uxtab16', 'sxtab16'])} {rd}, {rn}, {rm}{rot}",
            f"{par} {rd}, {rn}, {rm}",
            f"{par} {rd}, {rn}, {rm}\nsel {reg()}, {reg()}, {reg()}",
            f"pkhbt {rd}, {rn}, {rm}, lsl #{random.randrange(32)}",
            f"pkhtb {rd}, {rn}, {rm}, asr #{random.randrange(1, 33)}",
        ]))
    return out


def cases_multiply():
    out = []
    for _ in range(140):
        a, b, c, d = regs_distinct(4)
        out.append(random.choice([
            f"mul.w {a}, {b}, {c}",
            f"mla {a}, {b}, {c}, {d}",
            f"mls {a}, {b}, {c}, {d}",
            f"umull {a}, {b}, {c}, {d}",
            f"smull {a}, {b}, {c}, {d}",
            f"umlal {a}, {b}, {c}, {d}",
            f"smlal {a}, {b}, {c}, {d}",
            f"umaal {a}, {b}, {c}, {d}",
            f"sdiv {a}, {b}, {c}",
            f"udiv {a}, {b}, {c}",
            f"smul{random.choice(['bb', 'bt', 'tb', 'tt'])} {a}, {b}, {c}",
            f"smla{random.choice(['bb', 'bt', 'tb', 'tt'])} {a}, {b}, {c}, {d}",
            f"smmul{random.choice(['', 'r'])} {a}, {b}, {c}",
            f"smmla{random.choice(['', 'r'])} {a}, {b}, {c}, {d}",
        ]))
    return out


def cases_memory():
    out = []
    for _ in range(200):
        rt, rt2 = regs_distinct(2)
        off4 = 4 * random.randrange(15)
        off = random.randrange(0, 60)
        neg = random.randrange(1, 40)
        lst = sorted(random.sample(range(0, 11), random.randrange(1, 6)))
        rl = ', '.join('r%d' % r for r in lst)
        size = random.choice(['', 'b', 'h'])
        sgn = random.choice(['sb', 'sh'])
        sh = random.randrange(4)
        out.append(random.choice([
            f"ldr{size} {rt}, [r11, #{off4 if size == '' else off}]",
            f"str{size} {rt}, [r11, #{off4 if size == '' else off}]",
            f"ldr{sgn} {rt}, [r11, #{off}]",
            f"ldr{size}.w {rt}, [r11, #{off}]",
            f"movs r12, #{off & ~3}\nldr {rt}, [r11, r12]",
            f"movs r12, #{random.randrange(0, 14)}\nstr.w {rt}, [r11, r12, lsl #{sh}]",
            f"movs r12, #{random.randrange(0, 30)}\nldrb {rt}, [r11, r12]",
            f"movs r12, #{random.randrange(0, 30)}\nldrsh {rt}, [r11, r12]",
            f"adds r11, #{neg}\nldr {rt}, [r11, #-{neg}]",
            f"ldr {rt}, [r11, #{off4}]!",
            f"ldr {rt}, [r11], #{off4}",
            f"str {rt}, [r11, #{off4}]!",
            f"str {rt}, [r11], #{off4}",
            f"strb {rt}, [r11, #{off}]!",
            f"ldrh {rt}, [r11], #{off}",
            f"ldrsb {rt}, [r11, #{off}]!",
            f"ldrd {rt}, {rt2}, [r11, #{off4}]",
            f"ldrd {rt}, {rt2}, [r11, #{off4}]!",
            f"ldrd {rt}, {rt2}, [r11], #{off4}",
            f"strd {rt}, {rt2}, [r11, #{off4}]",
            f"strd {rt}, {rt2}, [r11, #{off4}]!",
            f"strd {rt}, {rt2}, [r11], #{off4}",
            f"ldmia r11, {{{rl}}}",
            f"ldmia r11!, {{{rl}}}",
            f"stmia r11, {{{rl}}}",
            f"stmia r11!, {{{rl}}}",
            f"adds r11, #32\nstmdb r11!, {{{rl}}}",
            f"adds r11, #32\nldmdb r11!, {{{rl}}}",
            f"mov sp, r11\nadd sp, sp, #32\npush {{{rl}}}\nmov r10, sp",
            f"mov sp, r11\nadd sp, sp, #8\npop {{{rl}}}\nmov r10, sp",
            f"mov sp, r11\nadd sp, sp, #32\nstr {rt}, [sp, #4]\nldr {rt2}, [sp, #8]\nmov r10, sp",
            f"ldr {rt}, =0x{random.getrandbits(32):08x}",
            f"adr.w {rt}, 1f\nnop\n1:",
        ]))
    return out


def cases_branches():
    out = []
    for i in range(120):
        c = cond()
        a, b = reg(), reg()
        n = random.randrange(0, 3)
        out.append(random.choice([
            f"cmp {a}, {b}\nb{c}.n 1f\nmovs r0, #1\n1:\nmovs r1, #2",
            f"cmp {a}, {b}\nb{c}.w 1f\nmovs r0, #1\n1:\nmovs r1, #2",
            f"cmp {a}, {b}\nb{c}.n 1f\nmovs r0, #1\nmovs r2, #4\n1:",
            f"cbz {reg(LOW)}, 1f\nmovs r0, #3\n1:\nmovs r1, #5",
            f"cbnz {reg(LOW)}, 1f\nmovs r0, #3\n1:\nmovs r1, #5",
            "b.n 1f\nmovs r0, #9\n1:\nmovs r1, #7",
            "b.w 1f\nmovs r0, #9\nmovs r2, #8\n1:\nmovs r1, #7",
            "bl 1f\nb.n 2f\n1:\nmovs r0, #4\nbx lr\n2:\nmovs r1, #6",
            f"movs r1, #{n}\ntbb [pc, r1]\n.Ltb{i}:\n.byte (.La{i}-.Ltb{i})/2, (.Lb{i}-.Ltb{i})/2, (.Lc{i}-.Ltb{i})/2, 0\n"
            f".align 1\n.La{i}:\nmovs r2, #1\nb.n .Ld{i}\n.Lb{i}:\nmovs r2, #2\nb.n .Ld{i}\n.Lc{i}:\nmovs r2, #3\n.Ld{i}:",
            f"movs r1, #{n}\ntbh [pc, r1, lsl #1]\n.Lth{i}:\n.short (.Lta{i}-.Lth{i})/2, (.Ltb{i}-.Lth{i})/2, (.Ltc{i}-.Lth{i})/2\n"
            f".align 1\n.Lta{i}:\nmovs r2, #1\nb.n .Ltd{i}\n.Ltb{i}:\nmovs r2, #2\nb.n .Ltd{i}\n.Ltc{i}:\nmovs r2, #3\n.Ltd{i}:",
        ]))
    return out


def cases_system():
    out = []
    for _ in range(40):
        a = reg()
        out.append(random.choice([
            f"mrs {a}, apsr",
            f"mrs {a}, xpsr",
            f"mrs {a}, ipsr",
            f"msr apsr_nzcvq, {a}",
            f"msr apsr_nzcvqg, {a}",
            f"msr apsr_nzcvq, {a}\nmrs {reg()}, apsr",
            "cpsid i\nmrs r0, primask\ncpsie i\nmrs r1, primask",
            "nop\ndmb\ndsb\nisb",
            f"mov.w {a}, #1\nmsr primask, {a}\nmrs r0, primask\nmovs r1, #0\nmsr primask, r1",
        ]))
    return out


FSPECIAL = [0x00000000, 0x80000000, 0x3f800000, 0xbf800000, 0x40000000, 0x3f000000, 0x40400000,
            0x3dcccccd, 0x42c80000, 0x7149f2ca, 0xf149f2ca, 0x0da24260, 0x00800000, 0x00000001,
            0x007fffff, 0x7f7fffff, 0x7f800000, 0xff800000, 0x7fc00000, 0x7f800001, 0x7fc12345,
            0xff800001, 0x80000001, 0x3fc00000, 0x4b000000, 0x4effffff, 0xcf000000, 0x40490fdb,
            0x34000000, 0x7e967699]


def fv():
    """Valeur initiale de registre flottant : souvent particulière, parfois aléatoire."""
    return random.choice(FSPECIAL) if random.random() < 0.7 else random.getrandbits(32)


def sreg(hi=16):
    return 's%d' % random.randrange(hi)


def dreg(hi=8):
    return 'd%d' % random.randrange(hi)


FP_IMMEDIATES = ['1.0', '-1.0', '0.5', '2.0', '31.0', '-2.5', '0.125', '1.5', '10.0', '0.25']


def ite_after_compare(d, m):
    c = cond()
    return (f"vcmp.f32 {d}, {m}\nvmrs APSR_nzcv, fpscr\nite {c}\nmov{c} r1, #1\n"
            f"mov{INVERSE[c]} r2, #2")


def cases_fp_arith():
    out = []
    ops3 = ['vadd', 'vsub', 'vmul', 'vdiv', 'vnmul', 'vmla', 'vmls', 'vnmla', 'vnmls', 'vfma',
            'vfms', 'vfnma', 'vfnms']
    for _ in range(200):
        d, n, m = sreg(), sreg(), sreg()
        out.append(random.choice([
            f"{random.choice(ops3)}.f32 {d}, {n}, {m}",
            f"{random.choice(ops3)}.f32 {d}, {n}, {m}",
            f"{random.choice(['vabs', 'vneg', 'vsqrt', 'vmov'])}.f32 {d}, {m}",
            f"vmov.f32 {d}, #{random.choice(FP_IMMEDIATES)}",
            f"{random.choice(['vcmp', 'vcmpe'])}.f32 {d}, {m}\nvmrs APSR_nzcv, fpscr",
            f"{random.choice(['vcmp', 'vcmpe'])}.f32 {d}, #0.0\nvmrs APSR_nzcv, fpscr",
            ite_after_compare(d, m),
            f"{random.choice(['vcvt', 'vcvtr'])}.{random.choice(['s32', 'u32'])}.f32 {d}, {m}",
            f"vcvt.f32.{random.choice(['s32', 'u32'])} {d}, {m}",
        ]))
    return out


def cases_fp_core_and_status():
    out = []
    for _ in range(120):
        a, b = regs_distinct(2, list(range(0, 11)))
        first = 2 * random.randrange(8)
        out.append(random.choice([
            f"vmov {a}, {sreg(32)}",
            f"vmov {sreg(32)}, {a}",
            f"vmov {a}, {b}, s{first}, s{first + 1}",
            f"vmov s{first}, s{first + 1}, {a}, {b}",
            f"vmov {dreg()}, {a}, {b}",
            f"vmov {a}, {b}, {dreg()}",
            f"vmrs {a}, fpscr",
            f"ldr r12, =0x0300009f\nand {a}, {a}, r12\nvmsr fpscr, {a}\nvmrs {b}, fpscr",
            "vmrs APSR_nzcv, fpscr",
            f"cmp {a}, {b}\nit eq\nvaddeq.f32 s0, s1, s2",
            f"cmp {a}, {b}\nite eq\nvmuleq.f32 s0, s1, s2\nvsubne.f32 s3, s4, s5",
            f"cmp {a}, {b}\nit ne\nvmovne {a}, s7",
            f"cmp {a}, {b}\nit hi\nvmovhi s9, {a}",
        ]))
    return out


def rng(prefix, first, count):
    """Liste de registres `{s4-s7}` ; un seul registre s'écrit sans tiret."""
    return f"{{{prefix}{first}}}" if count == 1 else f"{{{prefix}{first}-{prefix}{first + count - 1}}}"


def cases_fp_memory():
    out = []
    for _ in range(140):
        off4 = 4 * random.randrange(14)
        d = 2 * random.randrange(8)
        cnt = random.randrange(1, 7)
        dcnt = random.randrange(1, 4)
        out.append(random.choice([
            f"vldr {sreg(32)}, [r11, #{off4}]",
            f"vstr {sreg(32)}, [r11, #{off4}]",
            f"vldr {dreg(8)}, [r11, #{off4}]",
            f"vstr {dreg(8)}, [r11, #{off4}]",
            f"vldmia r11, {rng('s', d, cnt)}",
            f"vldmia r11!, {rng('s', d, cnt)}",
            f"vstmia r11, {rng('s', d, cnt)}",
            f"vstmia r11!, {rng('s', d, cnt)}",
            f"adds r11, #40\nvstmdb r11!, {rng('s', d, cnt)}",
            f"adds r11, #40\nvldmdb r11!, {rng('s', d, cnt)}",
            f"vldmia r11!, {rng('d', d // 2, dcnt)}",
            f"vstmia r11, {rng('d', d // 2, dcnt)}",
            f"mov sp, r11\nadd sp, sp, #40\nvpush {rng('s', d, cnt)}\nmov r10, sp",
            f"mov sp, r11\nadd sp, sp, #8\nvpop {rng('s', d, cnt)}\nmov r10, sp",
            f"mov sp, r11\nadd sp, sp, #40\nvpush {rng('d', d // 2, dcnt)}\nmov r10, sp",
            f"mov sp, r11\nadd sp, sp, #8\nvpop {rng('d', d // 2, dcnt)}\nmov r10, sp",
        ]))
    return out


DSPECIAL = [
    0x0000000000000000, 0x8000000000000000, 0x3ff0000000000000, 0xbff0000000000000,
    0x3fe0000000000000, 0x4000000000000000, 0x4008000000000000, 0x7ff0000000000000,
    0xfff0000000000000, 0x7ff8000000000000, 0x7ff0000000000001, 0xfff8000000000123,
    0x7fefffffffffffff, 0x0010000000000000, 0x000fffffffffffff, 0x0000000000000001,
    0x3fb999999999999a, 0x400921fb54442d18, 0x41e0000000000000, 0xc1e0000000000000,
    0x41efffffffe00000, 0x4202a05f20000000, 0x3ff8000000000000, 0x4004000000000000,
    0xbfe0000000000000, 0x7e37e43c8800759c, 0x47efffffe0000000, 0x3810000000000000,
    0x380fffffc0000000, 0x40f0000000000000, 0x40e0ffe000000000, 0x3f10000000000000,
    0x3e70000000000000, 0xc0f0000000000000, 0x40effc0000000000, 0x3f1fffe000000000]


def dval():
    return random.choice(DSPECIAL) if random.random() < 0.75 else random.getrandbits(64)


def load_doubles(*names):
    """Charge des doubles particuliers ou aléatoires dans les registres `dN` donnés."""
    lines = []
    for name in dict.fromkeys(names):
        v = dval()
        lines.append(f"ldr r0, =0x{v & 0xffffffff:08x}\nldr r1, =0x{v >> 32:08x}\nvmov {name}, r0, r1")
    return "\n".join(lines) + "\n"


def set_rounding():
    """Fixe parfois un mode d'arrondi de FPSCR (bits 23-22) pour les conversions."""
    rm = random.randrange(4)
    return (f"vmrs r0, fpscr\nbic r0, r0, #0xc00000\norr r0, r0, #{rm << 22}\nvmsr fpscr, r0\n"
            if rm else "")


def cases_fp_v8():
    out = []
    for _ in range(160):
        d, n, m = sreg(), sreg(), sreg()
        out.append(set_rounding() + random.choice([
            f"vsel{random.choice(['eq', 'vs', 'ge', 'gt'])}.f32 {d}, {n}, {m}",
            f"vsel{random.choice(['eq', 'vs', 'ge', 'gt'])}.f32 {d}, {n}, {m}",
            f"{random.choice(['vmaxnm', 'vminnm'])}.f32 {d}, {n}, {m}",
            f"{random.choice(['vmaxnm', 'vminnm'])}.f32 {d}, {n}, {m}",
            f"vrint{random.choice('anpmzrx')}.f32 {d}, {m}",
            f"vrint{random.choice('anpmzrx')}.f32 {d}, {m}",
            f"vcvt{random.choice('anpm')}.{random.choice(['s32', 'u32'])}.f32 {d}, {m}",
        ]))
    return out


def cases_fp_double():
    out = []
    ops3 = ['vadd', 'vsub', 'vmul', 'vdiv', 'vnmul', 'vmla', 'vmls', 'vnmla', 'vnmls', 'vfma',
            'vfms', 'vfnma', 'vfnms']
    for _ in range(250):
        d, n, m = dreg(), dreg(), dreg()
        s = sreg(32)
        sd = sreg(32)
        lane = random.randrange(2)
        c = cond()
        out.append(set_rounding() + random.choice([
            load_doubles(d, n, m) + f"{random.choice(ops3)}.f64 {d}, {n}, {m}",
            load_doubles(d, n, m) + f"{random.choice(ops3)}.f64 {d}, {n}, {m}",
            load_doubles(d, n, m) + f"{random.choice(ops3)}.f64 {d}, {n}, {m}",
            load_doubles(d, m) + f"{random.choice(['vabs', 'vneg', 'vsqrt', 'vmov'])}.f64 {d}, {m}",
            f"vmov.f64 {d}, #{random.choice(FP_IMMEDIATES)}",
            load_doubles(d, m) + f"{random.choice(['vcmp', 'vcmpe'])}.f64 {d}, {m}\nvmrs APSR_nzcv, fpscr",
            load_doubles(d) + f"{random.choice(['vcmp', 'vcmpe'])}.f64 {d}, #0.0\nvmrs APSR_nzcv, fpscr",
            load_doubles(d, m) + f"vcmp.f64 {d}, {m}\nvmrs APSR_nzcv, fpscr\nite {c}\nmov{c} r2, #1\nmov{INVERSE[c]} r3, #2",
            load_doubles(m) + f"{random.choice(['vcvt', 'vcvtr'])}.{random.choice(['s32', 'u32'])}.f64 {sd}, {m}",
            f"vcvt.f64.{random.choice(['s32', 'u32'])} {d}, {s}",
            load_doubles(m) + f"vcvt.f32.f64 {sd}, {m}",
            f"vcvt.f64.f32 {d}, {s}",
            load_doubles(d, n, m) + f"vsel{random.choice(['eq', 'vs', 'ge', 'gt'])}.f64 {d}, {n}, {m}",
            load_doubles(d, n, m) + f"{random.choice(['vmaxnm', 'vminnm'])}.f64 {d}, {n}, {m}",
            load_doubles(d, m) + f"vrint{random.choice('anpmzrx')}.f64 {d}, {m}",
            load_doubles(m) + f"vcvt{random.choice('anpm')}.{random.choice(['s32', 'u32'])}.f64 {sd}, {m}",
            f"vmov.32 {d}[{lane}], r{random.randrange(2, 8)}",
            f"vmov.32 r{random.randrange(2, 8)}, {d}[{lane}]",
        ]))
    return out


def cases_fp_half_and_fixed():
    out = []
    for _ in range(240):
        d, s, sd = dreg(), sreg(32), sreg(32)
        fb32, fb16 = random.randrange(1, 33), random.randrange(1, 17)
        out.append(set_rounding() + random.choice([
            f"vcvt{random.choice('bt')}.f16.f32 {sd}, {s}",
            f"vcvt{random.choice('bt')}.f16.f32 {sd}, {s}",
            f"vcvt{random.choice('bt')}.f32.f16 {sd}, {s}",
            f"vcvt{random.choice('bt')}.f32.f16 {sd}, {s}",
            load_doubles(d) + f"vcvt{random.choice('bt')}.f16.f64 {sd}, {d}",
            f"vcvt{random.choice('bt')}.f64.f16 {d}, {s}",
            f"vcvt.{random.choice(['s32', 'u32'])}.f32 {sd}, {sd}, #{fb32}",
            f"vcvt.{random.choice(['s16', 'u16'])}.f32 {sd}, {sd}, #{fb16}",
            f"vcvt.f32.{random.choice(['s32', 'u32'])} {sd}, {sd}, #{fb32}",
            f"vcvt.f32.{random.choice(['s16', 'u16'])} {sd}, {sd}, #{fb16}",
        ]))
    return out


FAMILIES = [
    ('thumb16', cases_thumb16),
    ('it_blocks', cases_it_blocks),
    ('thumb32_dp', cases_thumb32_dp),
    ('bitfield', cases_bitfield),
    ('extend_parallel', cases_extend_parallel),
    ('multiply', cases_multiply),
    ('memory', cases_memory),
    ('branches', cases_branches),
    ('system', cases_system),
    ('fp_arith', cases_fp_arith),
    ('fp_core_and_status', cases_fp_core_and_status),
    ('fp_memory', cases_fp_memory),
    ('fp_v8', cases_fp_v8),
    ('fp_double', cases_fp_double),
    ('fp_half_and_fixed', cases_fp_half_and_fixed),
]

# ---------------------------------------------------------------------------------------------
# Assemblage des lots, exécution par QEMU
# ---------------------------------------------------------------------------------------------

PROLOGUE = f"""
  ldr r12, =.Lbufinit
  ldr r11, ={BUF:#x}
  ldmia r12!, {{r0-r7}}
  stmia r11!, {{r0-r7}}
  ldmia r12!, {{r0-r7}}
  stmia r11!, {{r0-r7}}
  ldr r12, =.Linit@I@
  ldr r0, [r12, #48]
  msr APSR_nzcvqg, r0
  ldmia r12, {{r0-r11}}
"""

EPILOGUE = f"""
  ldr r12, ={OUT:#x}+@I@*{CASE_WORDS * 4}
  stmia r12!, {{r0-r11}}
  mrs r0, apsr
  str r0, [r12], #4
  ldr r0, ={BUF:#x}
  ldmia r0!, {{r1-r8}}
  stmia r12!, {{r1-r8}}
  ldmia r0!, {{r1-r8}}
  stmia r12!, {{r1-r8}}
"""


FP_PROLOGUE = f"""
  ldr r12, =.Lbufinit
  ldr r11, ={BUF:#x}
  ldmia r12!, {{r0-r7}}
  stmia r11!, {{r0-r7}}
  ldmia r12!, {{r0-r7}}
  stmia r11!, {{r0-r7}}
  ldr r12, =.Linit@I@
  ldr r0, [r12, #48]
  msr APSR_nzcvqg, r0
  ldr r0, [r12, #52]
  vmsr fpscr, r0
  add r0, r12, #56
  vldmia r0, {{s0-s31}}
  ldmia r12, {{r0-r11}}
"""

FP_EPILOGUE = f"""
  ldr r12, ={OUT:#x}+@I@*{FP_CASE_WORDS * 4}
  stmia r12!, {{r0-r11}}
  mrs r0, apsr
  str r0, [r12], #4
  vmrs r0, fpscr
  str r0, [r12], #4
  vstmia r12!, {{s0-s31}}
  ldr r0, ={BUF:#x}
  ldmia r0!, {{r1-r8}}
  stmia r12!, {{r1-r8}}
  ldmia r0!, {{r1-r8}}
  stmia r12!, {{r1-r8}}
"""

FPU_ENABLE = f"""
  ldr r0, =0xe000ed88
  ldr r1, [r0]
  orr r1, r1, #0xf00000
  str r1, [r0]
  dsb
  isb
"""


def assemble_batch(cases, fp=False, inits=None):
    lines = [
        '.syntax unified', '.cpu cortex-m7', '.fpu fpv5-d16', '.thumb',
        '.section .vectors,"a"', '  .word 0x20020000', '  .word _start+1', '  .rept 14',
        '  .word fault+1', '  .endr', '.text',
        '.thumb_func', '_start:', '  b.w .Lenter',
        '.thumb_func', 'fault:', '  b .',
        '.align 2', '.Lbufinit:',
    ]
    pattern = bytes((i * 7 + 0x13) & 0xff for i in range(64))
    for k in range(0, 64, 4):
        lines.append('  .word 0x%08x' % int.from_bytes(pattern[k:k + 4], 'little'))
    lines += ['.thumb_func', '.Lenter:']
    if fp:
        lines.append(FPU_ENABLE)
    words = FP_CASE_WORDS if fp else CASE_WORDS
    for i, case in enumerate(cases):
        lines.append(f'.Lcase{i}:')
        lines.append((FP_PROLOGUE if fp else PROLOGUE).replace('@I@', str(i)))
        lines.append(case)
        lines.append((FP_EPILOGUE if fp else EPILOGUE).replace('@I@', str(i)))
        lines.append(f'  b.w .Lcase{i + 1}')
        lines.append('.align 2')
        lines.append(f'.Linit{i}:')
        init = [rv() for _ in range(11)] + [BUF, rv() & 0xf80f0000]
        if fp:
            init += [random.choice([0, 1 << 24, 1 << 25, 3 << 24])] + [fv() for _ in range(32)]
        if inits is not None:
            inits.append(init)
        for value in init:
            lines.append('  .word 0x%08x' % value)
        lines.append('.ltorg')
    lines += [
        f'.Lcase{len(cases)}:',
        f'  ldr r0, ={OUT:#x}+{len(cases)}*{words * 4}',
        f'  ldr r1, ={MARKER:#x}',
        '  str r1, [r0]',
        '  bkpt #0',
        '.ltorg',
    ]
    return '\n'.join(lines) + '\n'


LINKER = """MEMORY { ROM (rx): ORIGIN = 0x0, LENGTH = 0x40000 }
SECTIONS { .text : { KEEP(*(.vectors)) *(.text*) *(.rodata*) } > ROM }
"""


def tool(name, env):
    return shlex.split(os.environ.get(env, name))


def run_qemu(elf, nbytes, work):
    out = os.path.join(work, 'qemu_out.bin')
    cmd = (tool('qemu-system-arm', 'QEMU') + shlex.split(os.environ.get('QEMU_ARGS', '')) +
           ['-M', 'mps2-an500', '-cpu', 'cortex-m7', '-nographic', '-monitor', 'stdio',
            '-serial', 'none', '-kernel', elf])
    for wait in (1.5, 4.0, 10.0):
        if os.path.exists(out):
            os.remove(out)
        p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                             stderr=subprocess.DEVNULL, text=True)
        time.sleep(wait)
        p.stdin.write('pmemsave %#x %d "%s"\nquit\n' % (OUT, nbytes, out))
        p.stdin.flush()
        try:
            p.wait(timeout=20)
        except subprocess.TimeoutExpired:
            p.kill()
        if os.path.exists(out):
            data = open(out, 'rb').read()
            if len(data) == nbytes and int.from_bytes(data[-4:], 'little') == MARKER:
                return data
    raise SystemExit("QEMU n'a pas terminé le programme (marqueur absent)")


def build_and_run(name, cases, outdir, fp=False):
    work = tempfile.mkdtemp(prefix='nwasabi-vec-')
    asm = os.path.join(work, 'prog.s')
    with open(asm, 'w') as f:
        inits = []
        f.write(assemble_batch(cases, fp, inits))
    with open(os.path.join(work, 'link.ld'), 'w') as f:
        f.write(LINKER)
    r = subprocess.run(tool('arm-none-eabi-as', 'ARM_AS') +
                       ['-mcpu=cortex-m7', '-mfpu=fpv5-d16', '-o', os.path.join(work, 'prog.o'), asm],
                       capture_output=True, text=True)
    if r.returncode:
        sys.stderr.write(r.stderr)
        raise SystemExit(f'assemblage de {name} impossible (programme laissé dans {asm})')
    subprocess.run(tool('arm-none-eabi-ld', 'ARM_LD') +
                   ['-T', os.path.join(work, 'link.ld'), '-o', os.path.join(work, 'prog.elf'),
                    os.path.join(work, 'prog.o')], check=True)
    binfile = os.path.join(work, 'prog.bin')
    subprocess.run(tool('arm-none-eabi-objcopy', 'ARM_OBJCOPY') +
                   ['-O', 'binary', os.path.join(work, 'prog.elf'), binfile], check=True)
    data = run_qemu(os.path.join(work, 'prog.elf'), len(cases) * (FP_CASE_WORDS if fp else CASE_WORDS) * 4 + 4, work)
    os.makedirs(outdir, exist_ok=True)
    with open(binfile, 'rb') as src, open(os.path.join(outdir, name + '.bin'), 'wb') as dst:
        dst.write(src.read())
    with open(os.path.join(outdir, name + '.expected'), 'wb') as f:
        f.write(data)
    with open(os.path.join(outdir, name + '.cases'), 'w') as f:
        for i, c in enumerate(cases):
            f.write(f'#{i}\n{c}\n')
            f.write('; init : ' + ' '.join('%08x' % v for v in inits[i]) + '\n')
    print(f'{name}: {len(cases)} cas, programme de {os.path.getsize(binfile)} octets')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', default='tests/golden/cpu')
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--only', default='')
    args = ap.parse_args()
    random.seed(args.seed)
    for n, (fam, fn) in enumerate(FAMILIES):
        cases = fn()   # toujours générés, pour que le choix de --only ne change pas les valeurs
        if args.only and args.only not in fam:
            continue
        build_and_run('%02d_%s' % (n, fam), cases, args.out, fp=fam.startswith('fp_'))


if __name__ == '__main__':
    main()
