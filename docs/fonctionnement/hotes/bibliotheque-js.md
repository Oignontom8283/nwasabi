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

1. **cadence** : si l'écran est plus rapide que 60 Hz (`now − last < FRAME_MS − 1`), l'image est sautée ;
2. **budget de temps** : `min(temps réel écoulé, maxElapsedMs)`. C'est le temps que l'application **voit passer** : elle ne voit jamais plus de temps qu'il n'en est réellement écoulé, et rien en pause (`stop()`) ;
3. **instructions permises** : avec une fréquence (`limitMhz`, modes `model` ou `custom`), `room = floor(MHz × budget × 1000)` ; sans limite (`max`), autant que possible ;
4. **`run { instructions: min(slice, room), frames: 1 }`** en boucle (au plus `MAX_RUNS_PER_TICK` fois) : après chaque tranche, le temps que le processeur a mis à exécuter les instructions (`exécutées / (MHz × 1000)`) est **donné à l'application** (`advance`) et
   retiré du budget ;
5. selon l'arrêt : `sleep` → le temps passe pour l'application dans la limite du budget, et la boucle continue s'il en reste ; `budget` → continue tant qu'il reste du temps de processeur (ou sans limite de vitesse) et que `maxWallMs` de temps réel n'est pas dépassé, sinon `capHits`
   est incrémenté (l'application n'a pas pu tourner à la vitesse émulée) ; `frame`, `wait_event` → le budget restant est donné à l'application (`advance(budget)`) et l'image se termine ; tout autre arrêt (`exited`, `fault`, `breakpoint`, `watchpoint`, `home_pressed`…)
   **arrête le lecteur** (`running = false`) ;
6. **finitions** : `timeMs`, dessin de l'écran, `onFrame`, lecture de la sortie standard toutes les 10 images, puis replanification (ou `bump()` si arrêté).

Le noyau est en **horloge hôte** : `advance` reçoit le temps réellement écoulé, et rien pendant une pause. `PlayerState` expose entre autres `loaded`, `running`, `model`, `firmware`, `speed`, `customMhz`, `limitMhz`, `appName`, `stop`, `frames`, `instructions`, `error`, `stdout`,
`timeMs`, `keymapName`, `needsData` (l'application demande des données externes qu'on ne lui a pas données : elle n'est pas lancée), `revision` (augmente quand les vues dérivées doivent se rafraîchir) et `capHits`.

**Touches** : une touche enfoncée puis relâchée entre deux images serait perdue pour une application qui ne lit le clavier qu'une fois par image. Le lecteur note donc les touches **échantillonnées** (`sampled`) par une tranche et **diffère** (`deferred`) le relâchement d'une touche déjà
relâchée jusqu'à ce qu'une tranche l'ait vue.

## Écran

`attach(canvas)` dessine dans un canvas 2D (`HTMLCanvasElement`, `OffscreenCanvas` ou tout objet ressemblant : `CanvasLike`) à chaque image (`createImageData` puis `putImageData`, la conversion RGB565 → RGBA étant faite dans le module par `drawTo`). Sans canvas, `framebuffer()` donne les pixels RGB565 bruts et
`rgba()` une copie RGBA 8 bits.

## Entrées

`InputMapper` traduit un `KeyboardEvent` (position `code` ou caractère `key`) en touches de la calculatrice d'après le `KeyMap`, avec la logique des **accords** (une touche de l'ordinateur → plusieurs touches de la calculatrice) ; `attachPointer` ajoute la reconnaissance de **gestes** tactiles (`gestures.ts`) liés par les règles du fichier de mapping. La répétition des touches est faite par le **noyau** ([Clavier](../noyau/clavier.md)), pas par la bibliothèque.
