# @nwasabi/js

Pilote le module WebAssembly de [nwasabi](https://github.com/Oignontom8283/nwasabi), un émulateur
d'applications externes NumWorks (`.nwa`) : parler au module, lire son écran, envoyer les touches et
faire tourner l'application image par image dans un navigateur.

Ce paquet **n'embarque pas** le module `.wasm` : il faut le lui fournir. Le paquet
[`@nwasabi/wasm`](https://www.npmjs.com/package/@nwasabi/wasm) contient celui de la même version, mais
vous pouvez aussi donner le vôtre.

Il fonctionne dans un navigateur comme dans Node.js : rien n'y suppose un canvas ni
`requestAnimationFrame`. L'écran sort en pixels bruts (RGB565 ou RGBA), et le rythme peut venir de
n'importe quel `Scheduler` (par défaut `requestAnimationFrame`, sinon des minuteries à 60 Hz).

Navigateur (avec dessin dans un canvas) :

```ts
import { Player } from "@nwasabi/js";
import { wasmUrl } from "@nwasabi/wasm";

const player = await Player.create(await fetch(wasmUrl));
player.attach(document.querySelector("canvas")!);
player.load(new Uint8Array(await file.arrayBuffer()), file.name);
```

Node.js (pixels bruts, sans canvas) :

```ts
import { readFileSync } from "node:fs";
import { Player } from "@nwasabi/js";

const player = await Player.create(readFileSync("nwasabi.wasm"), {
  onFrame: (frame) => handle(frame.rgba()), // 320×240×4 octets, ou frame.rgb565
});
player.load(readFileSync("app.nwa"), "app.nwa");
```

## Vitesse du processeur

Par défaut, l'application s'exécute à la fréquence du modèle émulé (192 MHz sur N0110 et N0115, 540 MHz
sur N0120). Trois modes, que l'on peut changer à tout moment sans relancer l'application :

```ts
player.setSpeed("custom"); // "model" (défaut), "custom" ou "max"
player.setCustomMhz(96);   // la fréquence du mode "custom", en MHz (192 au départ, celle du N0110)
```

- `"model"` : la fréquence du modèle ;
- `"custom"` : la fréquence donnée par `setCustomMhz` ; elle est **ignorée** dans les autres modes ;
- `"max"` : aucune limite, l'application s'exécute au plus vite (elle reste rythmée par les 60 images
  par seconde de l'écran).

Le mode et la fréquence peuvent aussi être donnés à la création (`Player.create(wasm, { speed: "custom",
customMhz: 96 })`). `player.state.limitMhz` donne la fréquence appliquée en ce moment (`null` sans limite).
Ce que l'application exécute vraiment par seconde ne peut pas dépasser ce que l'ordinateur sait faire :
une fréquence très élevée n'est atteinte que si la machine est assez puissante.

Documentation, code source et suivi des problèmes : <https://github.com/Oignontom8283/nwasabi>.

## Touches : clavier et gestes tactiles (`.nmk`)

Un mapping décrit comment le clavier de l'ordinateur et les gestes tactiles deviennent des touches de la
calculatrice (format `.nmk`, JSON : voir `docs/format/nmk.md` dans le dépôt). Sans rien faire, le mapping par défaut
s'applique : flèches, chiffres, Entrée = OK… et un balayage vers la gauche, la droite, le haut ou le bas
appuie sur la flèche correspondante.

```ts
import { KeyMap, Player } from "@nwasabi/js";

const player = await Player.create(fetch(wasmUrl));
player.input.attachKeyboard(window);            // clavier de l'ordinateur
player.input.attachPointer(canvas);             // gestes sur l'écran (CSS : touch-action: none)

// Un mapping de jeu, lu depuis un fichier .nmk (lève NmkError, avec le chemin de l'erreur, s'il est invalide).
player.setKeyMap(KeyMap.parse(await (await fetch("jeu.nmk")).text()));
```

`player.input` est un `InputMapper` : `hostKey(event, down)` (clavier) et `pointerDown/Move/Up/Cancel` (gestes)
permettent de brancher soi-même les événements ; `KeyMap.default()` donne le mapping par défaut, et
`player.state.keymapName` le nom du mapping en vigueur. Quand plusieurs entrées visent la même touche de la
calculatrice, elle reste enfoncée tant qu'une d'elles est tenue.

## Licence

MIT ou Apache-2.0, au choix (voir [LICENSE/MIT](./LICENSE/MIT) et [LICENSE/APACHE](./LICENSE/APACHE)).
