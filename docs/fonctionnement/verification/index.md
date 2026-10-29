---
title: "Vérification"
description: "Comment Nwasabi est vérifié : vecteurs de test du processeur produits par QEMU, applications C de test, corpus de tiers, références, parité native et WebAssembly, GDB réel, tests du site."
---

# Vérification

Un émulateur n'a de valeur que s'il est juste. Les contrôles de Nwasabi sont à plusieurs étages, du plus petit (une instruction) au plus grand (une application de tiers, un vrai GDB). Aucun test ne dépend du réseau ni d'un fichier de NumWorks.

## 1. Le processeur contre QEMU

`tests/cpu/generate_vectors.py` écrit de petits programmes Thumb-2 qui enchaînent des **centaines de cas** par lot : chaque cas charge des valeurs initiales dans `r0`–`r11` et dans les drapeaux, exécute quelques instructions, puis recopie les registres,
`APSR` et un tampon de 64 octets dans une zone de résultats (`OUT = 0x2001_0000`). **QEMU** (machine `mps2-an500`, Cortex-M7) exécute le programme : c'est l'**oracle**. `tests/cpu_vectors.rs` rejoue les mêmes programmes sur `cpu::Cpu` et compare **octet pour octet**
(un marqueur `0x00C0FFEE` final détecte une exécution tronquée). Les valeurs initiales sont des valeurs limites (`0`, `1`, `0x7FFFFFFF`, `0x80000000`, `0xFFFFFFFF`, motifs `0x55555555`…) pour 60 % et aléatoires sinon, avec une graine fixe. Les lots flottants (`FP_CASE_WORDS = 62`) ajoutent
`FPSCR` et `s0`–`s31`. Les fichiers `.bin`, `.expected` et `.cases` (le texte de chaque cas, pour lire les différences) sont versionnés dans `tests/golden/cpu/` ; QEMU n'est nécessaire que pour **régénérer**.

`tests/cpu_independence.rs` vérifie que `src/cpu/` n'utilise rien d'autre dans la caisse. Les tests unitaires de `cpu/tests.rs` (655 lignes) couvrent les cas que les vecteurs ne couvrent pas : exceptions, exclusivité, blocs `IT`.

Une revue systématique de **tous les encodages** Thumb-2 de l'ARMv7E-M a été faite : le décodeur et le désassembleur reconnaissent la même chose, et tout ce que le Cortex-M7 ne sait pas faire (coprocesseurs, NEON, mode ARM) donne `Unimplemented`.

## 2. Applications de test compilées

`tests/apps/` contient de minuscules applications `.nwa` en **C** compilées par GCC (`just build-test-apps`, binaires versionnés dans `bin/`) qui **se vérifient elles-mêmes** (macro `CHECK`) : `integer.c` (division, multiplications longues, champs de bits, `rev`, `clz`, branchements de table),
`float.c` (simple précision), `keys.c`, `exclusive_flash.c` et `exclusive_ram.c` (`ldrex` accepté en flash, `BusFault` en RAM), `debugme.c` (pour le débogueur), `aux_flash.c` et `flash_rw.c` (données auxiliaires et programmation de la flash), `modifiers.c`. Elles prouvent qu'un vrai fichier
produit par un compilateur traverse **toute** la chaîne (lecture, liaison, exécution). Elles rendent 0 si tout va bien, sinon le numéro de la première ligne ratée.

## 3. Le corpus de tiers

Si le dossier `local/nwa` (ignoré par git) contient des `.nwa` de tiers, des tests les exploitent ; sinon ils sont **ignorés sans bruit**. `just test-corpus <dossier>` en donne un autre.

| Test | Ce qu'il vérifie |
|---|---|
| `emulator_corpus` | aucune application ne s'arrête sur une **faute d'émulation** (instruction ou appel système inconnu, accès invalide) et l'écran après un nombre fixe d'instructions est **toujours le même** (`tests/golden/screens.tsv`) |
| `link_corpus` | la taille et le CRC-32 de l'**image liée** de chaque application, et ses adresses principales (`link_images.tsv`) ; ces images ont été confrontées à `arm-none-eabi-ld` avec le même script : **identiques octet pour octet** à la fusion des chaînes près |
| `corpus_elf`, `device_corpus` | lecture ELF de toutes les applications ; installation de chacune dans un appareil fabriqué |
| `debugger_corpus` | les informations de débogage de vraies applications se lisent sans erreur, leurs adresses se retrouvent dans la table des lignes |

`just update-golden <dossier>` régénère les références après un changement voulu.

## 4. Le cœur par le haut

Tests d'intégration natifs (`tests/*.rs`) : `emulator_apps`, `emulator_exclusive`, `emulator_fs`, `emulator_memory`, `emulator_nested`, `session`, `settings`, `flash`, `apps_flash`, `formats`, `fixtures` (un fichier de chaque format), `inspect_structures`, `link_synthetic` (objets fabriqués
par `ObjectWriter`), `cli`, `server`, `gdb`, `debugger` (23 tests : expressions, appels, C++, Rust, historique des expressions), `gdb_real`.

**`gdb_real`** lance un **vrai GDB** (`gdb-multiarch` ou `arm-none-eabi-gdb`, variable `NWASABI_GDB`) contre `nwasabi --gdb-headless` avec l'ELF de l'application : point d'arrêt sur une fonction, pile, variables, lignes de source, `finish`, `watch`. Il est ignoré sans GDB. Trois projets de test
(C, C++, Rust), compilés à la main, ont servi à valider l'ensemble des expressions et des commandes contre un vrai GDB.

## 5. Native contre WebAssembly

`tests/wasm/*.mjs` (sous Node.js, `just test-wasm`) exécutent le **module WebAssembly** : `parity.mjs` rejoue les mêmes requêtes qu'en natif et compare les sommes de contrôle de l'écran, du nombre d'instructions et des arrêts ; `player.mjs` et `speed.mjs` vérifient la cadence du `Player` ;
`state.mjs`, `step.mjs`, `debug.mjs`, `flash.mjs`, `data.mjs` et `keymap.mjs` couvrent les états, le pas à pas, le débogage du code source, la flash, les données externes et les mappings (parité avec `keymap/` en Rust).

## 6. Le site et l'interface

`tests/web/*.mjs` : logique pure sous Node.js (console GDB, sélection, GIF, instantanés, recherche de valeurs, registres de faute, descriptions d'instructions, lancement), plus `locales.mjs`, qui vérifie que **chaque clé de traduction utilisée existe** dans les deux langues, et qu'aucune clé n'est inutilisée.
`just typecheck-web` (`vue-tsc`) vérifie les types ; `just build-docs` construit le site entier (VitePress, émulateur, schémas) et échoue sur un lien mort.

## Les commandes

| Recette | Contenu |
|---|---|
| `just ci` | formatage, clippy, tests natifs, WebAssembly, types de l'interface : **la séquence rapide** avant un changement |
| `just check-all [corpus]` | tout : en plus, compilation de tout, tests WebAssembly, émulateur, site de documentation, contenu des paquets npm |
| `just test`, `just test-wasm`, `just test-corpus` | les tests à part |

## Ce qui n'est pas vérifié

Il n'y a pas de test **contre une vraie calculatrice** : la fidélité d'un comportement du système (par exemple l'ordre des événements d'un appui long, ou la valeur d'un appel d'information) vient de relevés. Le **temps** est un modèle (un cycle par instruction) : une comparaison de vitesse avec la vraie
puce n'a pas de sens. L'interface graphique n'a pas de test automatique de bout en bout.
