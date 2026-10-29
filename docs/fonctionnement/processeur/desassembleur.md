---
title: "Désassembleur"
description: "Le désassembleur Thumb-2 de Nwasabi : syntaxe unifiée, suivi des blocs IT, cibles des branchements et usage par les vues de code et GDB."
---

# Désassembleur

`disasm.rs` met en texte les instructions que le décodeur reconnaît, en **syntaxe unifiée** d'ARM (`adds r0, r1, #4`, `ldr.w r2, [r3, #8]`). Il
ne dépend que du décodeur : il lit la mémoire par une fonction qui rend des demi-mots, donc il fonctionne sur la mémoire émulée comme sur un
tampon.

`cpu::disassemble` rend des `Line` :

| Champ | Contenu |
|---|---|
| `address`, `size` | adresse de l'instruction, 2 ou 4 octets |
| `halfwords` | les demi-mots de l'instruction (le second vaut 0 pour une instruction de 16 bits) |
| `mnemonic`, `operands` | le texte, séparé pour qu'une interface colore les deux |
| `target` | cible d'un branchement, ou adresse d'un mot lu relativement à `pc` (`ldr r0, [pc, #…]`) |

Les blocs `it` sont suivis le long d'une suite d'instructions : celles qu'ils rendent conditionnelles prennent leur **suffixe** (`addeq`,
`movne`). Une instruction hors de portée du décodeur apparaît comme `.hword` ou `.word` avec ses octets.

Il sert à la vue **Code** de la page, à la commande `disasm` de la [session](../debogage/session.md), à `x/Ni` et `disassemble` de la console et
à la pile estimée (reconnaître l'instruction qui précède une adresse de retour : `bl` ou `blx`).
