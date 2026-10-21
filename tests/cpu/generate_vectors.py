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


