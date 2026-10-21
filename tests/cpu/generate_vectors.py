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

