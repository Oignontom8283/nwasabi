# Technologies prises en charge

Ce que Nwasabi comprend et ce qu'il ne comprend pas. Les cases « testé » ont été essayées de bout en bout ; le reste est ce que le
format ou le jeu d'instructions autorise, sans essai particulier.

## Langages et compilateurs

| | État | Remarques |
|---|---|---|
| **C** | testé | GCC 14.2 (`arm-none-eabi-gcc`), C11 ; DWARF 5 |
| **C++** | testé | GCC 14.2 (`arm-none-eabi-g++`), C++14 ; sans exceptions, sans RTTI, sans objets globaux à constructeur |
| **Rust** | testé | `rustc` 1.96, cible `thumbv7em-none-eabihf`, `no_std` ; DWARF 4 |
| **Assembleur** | pris en charge | tout fichier objet ELF ARM Thumb-2 se lie |
| Autres langages (Zig, Go, Nim…) | non testé | possible s'ils produisent des objets ELF ARM Thumb-2 pour un Cortex-M7 |
| Clang / LLVM | non testé | |
| MicroPython, Lua | existent comme applications (`.nwa`) | ce sont des programmes C, exécutés comme les autres |

La chaîne de liaison est celle de NumWorks : l'application est un objet ELF relogeable (`-Wl,--relocatable`), que Nwasabi lie à la place du
système de la calculatrice ([Écrire une application](../guide/ecrire-une-application.md)).

## Processeur

| | État |
|---|---|
| Thumb-2, ARMv7E-M complet (Cortex-M7) | pris en charge ; vérifié contre QEMU et par balayage de tous les encodages |
| Extension DSP (arithmétique parallèle, saturations, doubles produits) | pris en charge |
| Virgule flottante FPv5, simple et double précision (D0 à D15), demi-précision, virgule fixe | pris en charge |
| Instructions de coprocesseur, NEON, registres D16 à D31 | non : elles n'existent pas sur un Cortex-M7 (instruction indéfinie) |
| Jeu d'instructions ARM (non Thumb), ARMv8-A | non |
| Exceptions, interruptions, gestionnaires, SysTick, MPU | non (voir [Compatibilité](./compatibilite.md)) |
| Cache, pipeline, temps d'accès à la flash | non : un cycle par instruction |

## Informations de débogage

| | État |
|---|---|
| DWARF 2 à 5 : lignes, fonctions, variables, types, blocs imbriqués | pris en charge |
| Tables d'appel `.debug_frame` (pile exacte) | pris en charge |
| Listes d'emplacements (variables qui changent de registre) | pris en charge |
| C : structures, unions, tableaux, énumérations, champs de bits, pointeurs de fonction | pris en charge |
| C++ : espaces de noms, classes, héritage, fonctions virtuelles, références, modèles, surcharges, membres statiques | pris en charge |
| Rust : structures, énumérations avec données, tranches, méthodes, traits | pris en charge |
| Noms lisibles : C++ (Itanium), Rust (ancien et v0) | pris en charge |
| `.eh_frame`, tables d'exceptions ARM (`.ARM.exidx`) | non : la pile est estimée sans `.debug_frame` |
| Sections de débogage compressées, DWARF séparé (`.dwo`, `.dwp`), fichiers de débogage externes | non |
| Code optimisé (`-O1` à `-O3`) | partiel : voir [Déboguer en C, C++ et Rust](../guide/deboguer-c-cpp-rust.md#ce-qui-ne-marche-pas-encore) |

## Outils

| | État |
|---|---|
| GDB, `gdb-multiarch`, `arm-none-eabi-gdb` (protocole Remote Serial, ELF exporté) | testé avec GDB 17 en C, C++ et Rust |
| LLDB, VS Code (extension qui pilote GDB) | non testé |
| Points d'arrêt par ligne, pas à la ligne, variables, expressions, appels de fonctions | pris en charge |
| Débogage en remontant le temps (`reverse-stepi`, `reverse-continue`) | pris en charge |
| Points d'observation | pris en charge (matériels, d'après la mémoire) |
| Plusieurs fils d'exécution, processus | non : une application, un fil |

## Fichiers

| | Format |
|---|---|
| Application | `.nwa` (objet ELF relogeable) |
| Mapping des touches, fichiers de la calculatrice, données auxiliaires en flash, état, enregistrement | `.nmk`, `.nfs`, `.nfd`, `.nds`, `.nrp` : [Formats de fichiers](../format/index.md) |
| Application liée pour GDB | ELF exécutable (`nwasabi-cli link --elf`) |

## Plateformes

| | État |
|---|---|
| Serveur et ligne de commande : Linux, macOS, Windows | `cargo install nwasabi` (Rust 2024 ; Node.js et la cible `wasm32-unknown-unknown` pour compiler l'interface) |
| Navigateur | WebAssembly avec SIMD et opérations mémoire en bloc : Chrome ou Edge 91+, Firefox 89+, Safari 16.4+ |
| Calculatrices | N0110, N0115, N0120 |
| Micrologiciels | dispositions `24` (appareils relevés) et `26.4.0` |

Pour ce qui concerne les **systèmes d'exploitation**, seul Linux (sous WSL) a servi aux essais de ce projet ; le code ne dépend d'aucun
système.
