---
title: "La bibliothèque JavaScript"
description: "Le Player de @nwasabi/js : cadence, planificateur, tranches, horloge hôte, entrées clavier et tactiles, canvas ou pixels bruts, et module @nwasabi/wasm."
---

# La bibliothèque JavaScript

`packages/js` (`@nwasabi/js`) est une bibliothèque TypeScript qui fait tourner le module WebAssembly dans un navigateur ou sous Node.js ; `packages/wasm` (`@nwasabi/wasm`) embarque le `.wasm` lui-même. Son usage est décrit dans la
[référence](../../reference/bibliotheque-js.md) ; voici comment `Player` est fait.

## Fichiers

| Fichier | Rôle |
|---|---|
| `nwasabi.ts` | l'enveloppe du module : `alloc`, `request(json)`, `framebuffer()`, `drawTo(imageData)` |
| `player.ts` | `Player` : la boucle de cadence, l'état (`PlayerState`), les entrées |
| `input.ts`, `gestures.ts` | clavier de l'ordinateur et gestes tactiles → touches |
| `keymap.ts` | lecture des [`.nmk`](../../reference/formats/nmk.md) (mêmes règles que `keymap/` en Rust) |
| `keys.ts` | noms des touches |

## `Player`

`Player.create(wasm, options)` crée le module et une session. Options : `scheduler` (par défaut `requestAnimationFrame` ou une minuterie Node), `slice` (instructions par tranche, 2 000 000 par défaut), `maxWallMs` (temps réel maximal passé à exécuter sans limite de vitesse, 12 ms par défaut), `maxElapsedMs` (garde-fou contre une image très en retard, 250 ms par défaut).

La boucle `tick(now)` (une « image du navigateur ») :

```mermaid
flowchart TD
  A(["tick(now)"]) --> B{"Trop tôt ?<br/>now − last &lt; 16,7 ms"}
  B -- oui --> S1["replanifier"]
  B -- non --> C["budget = min(temps réel écoulé, maxElapsedMs)<br/>(le temps que l'application va « voir »)"]
  C --> D["room = MHz × budget × 1000<br/>(∞ sans limite de vitesse)"]
  D --> E{"room &lt; 1 ?"}
  E -- oui --> F["advance(budget)"] --> Z
  E -- non --> G["run { instructions: min(slice, room), frames: 1 }"]
  G --> H["donner à l'application le temps passé<br/>à exécuter : advance(exécutées / MHz)"]
  H --> I{"stop.kind"}
  I -- sleep --> J["advance(min(durée demandée, budget))"] --> K{"Il reste du budget ?"}
  K -- oui --> E
  K -- non --> Z
  I -- budget --> L{"Du temps de processeur reste<br/>et moins de maxWallMs écoulé ?"}
  L -- oui --> E
  L -- non --> M["capHits++ si limité<br/>advance(budget restant)"] --> Z
  I -- "frame, wait_event" --> N["advance(budget restant)"] --> Z
  I -- "exited, fault, breakpoint,<br/>watchpoint, home_pressed…" --> O["running = false"] --> Z
  Z["finitions : timeMs, dessin, onFrame,<br/>stdout toutes les 10 images"] --> S1
  S1(["image suivante"])
```

La figure en mots : le **budget** est le temps que l'application a le droit de voir passer. Elle le consomme en exécutant des instructions (à la vitesse émulée), en dormant ou en attendant une touche ; ce qui reste lui est donné à la fin de l'image.

Le noyau est en **horloge hôte** : `advance` reçoit le temps réellement écoulé, et rien pendant une pause. `PlayerState` expose entre autres `loaded`, `running`, `model`, `firmware`, `speed`, `customMhz`, `limitMhz`, `appName`, `stop`, `frames`, `instructions`, `error`, `stdout`,
`timeMs`, `keymapName`, `needsData` (l'application demande des données externes qu'on ne lui a pas données : elle n'est pas lancée), `revision` (augmente quand les vues dérivées doivent se rafraîchir) et `capHits`.

**Touches** : une touche enfoncée puis relâchée entre deux images serait perdue pour une application qui ne lit le clavier qu'une fois par image. Le lecteur note donc les touches **échantillonnées** (`sampled`) par une tranche et **diffère** (`deferred`) le relâchement d'une touche déjà
relâchée jusqu'à ce qu'une tranche l'ait vue.

## Écran

`attach(canvas)` dessine dans un canvas 2D (`HTMLCanvasElement`, `OffscreenCanvas` ou tout objet ressemblant : `CanvasLike`) à chaque image (`createImageData` puis `putImageData`, la conversion RGB565 → RGBA étant faite dans le module par `drawTo`). Sans canvas, `framebuffer()` donne les pixels RGB565 bruts et
`rgba()` une copie RGBA 8 bits.

## Entrées

`InputMapper` traduit un `KeyboardEvent` (position `code` ou caractère `key`) en touches de la calculatrice d'après le `KeyMap`, avec la logique des **accords** (une touche de l'ordinateur → plusieurs touches de la calculatrice) ; `attachPointer` ajoute la reconnaissance de **gestes** tactiles (`gestures.ts`) liés par les règles du fichier de mapping. La répétition des touches est faite par le **noyau** ([Clavier](../noyau/clavier.md)), pas par la bibliothèque.
