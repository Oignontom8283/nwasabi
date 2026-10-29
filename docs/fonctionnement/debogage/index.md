---
title: "Le débogage"
description: "Vue d'ensemble de la pile de débogage de Nwasabi : points d'arrêt, observation, DWARF, pile exacte ou estimée, expressions, historique déterministe, session JSON et serveur GDB."
---

# Le débogage

::: tip En bref
Tous les débogueurs (page web, GDB, ligne de commande) parlent à **la même session**, par des requêtes JSON. Il n'y a qu'un seul chemin d'exécution à maintenir, donc GDB voit exactement ce que voit la page.
:::


Le débogage de Nwasabi repose sur une idée : **une seule voie de contrôle**. La page web, `nwasabi-cli session`, la bibliothèque `@nwasabi/js` et le serveur GDB envoient tous des **requêtes JSON** à une `Session`, qui pilote un `Emulator`. Il n'y a
aucun second chemin d'exécution à maintenir : un débogueur GDB voit exactement ce que voit l'interface web.

```mermaid
flowchart LR
  gdb[GDB / IDE] -->|RSP| srv[gdb.rs]
  srv -->|JSON| ses[Session]
  page[Page web] -->|JSON| ses
  cli[nwasabi-cli] -->|JSON| ses
  ses --> emu[Emulator]
  ses --> src[session/source.rs]
  src --> dbg[debuginfo : lignes, fonctions, types, pile]
  src --> expr[expr.rs / eval.rs]
  ses --> his[history.rs]
  emu --> mem[Memory : observation]
```

## Les briques

| Brique | Rôle | Page |
|---|---|---|
| points d'arrêt (adresse, condition) et d'observation | `Emulator` et `Memory` | [Exécution](../execution/index.md), [Mémoire](../memoire/index.md) |
| `debuginfo` : lecture DWARF | lignes, fonctions, variables, types | [DWARF](./dwarf.md) |
| pile d'appels | CFI exacte, ou estimée sans informations | [Pile d'appels](./pile.md) |
| expressions | syntaxe C, affectations, appels de fonctions | [Expressions](./expressions.md) |
| C++ et Rust | noms qualifiés, classes, variantes, méthodes, noms lisibles | [C++ et Rust](./langages.md) |
| historique | remonter le temps par points de reprise et rejeu | [Historique](./historique.md) |
| session | protocole JSON, effets sur l'historique | [Session](./session.md) |
| GDB | Remote Serial Protocol, pont avec la page | [GDB](./gdb.md) |

## Sans informations de débogage

Sans `-g`, rien de ce qui précède (lignes, variables, types) n'existe. Restent : les points d'arrêt par adresse, l'observation, le pas à l'instruction, le désassemblage, les registres, la mémoire, la pile **estimée**, le profil et les
symboles du fichier `.nwa` (qui donnent les noms de fonctions). `step` et `next` deviennent des pas d'instruction.

## Choix de conception notables

- **Les expressions peuvent modifier l'appareil.** Une affectation ou un appel de fonction écrit dans la mémoire ou les registres ; la session le détecte (`EmuTarget::dirty`) et **remet l'historique à zéro**. Les expressions de lecture seule ne le font pas.
- **Un appel de fonction est exécuté sur l'émulateur**, avec un retour piégé par un point d'arrêt, puis le processeur et le noyau sont **restaurés** ; la mémoire garde les effets de l'appel, comme avec GDB.
- **Le rejeu ignore points d'arrêt et d'observation** (`suspend_debugging`) pour reconstituer un état exactement, puis les rétablit.
- **Pas de modèle de temps pour les exceptions** : une faute arrête l'application ; ses causes sont dans les [registres de faute](../memoire/appareil.md#registres-de-faute).
