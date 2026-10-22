# @nwasabi/js

Pilote le module WebAssembly de [nwasabi](https://github.com/Oignontom8283/nwasabi), un émulateur
d'applications externes NumWorks (`.nwa`) : parler au module, lire son écran, envoyer les touches et
faire tourner l'application image par image dans un navigateur.

Ce paquet **n'embarque pas** le module `.wasm` : il faut le lui fournir. Le paquet
[`@nwasabi/wasm`](https://www.npmjs.com/package/@nwasabi/wasm) contient celui de la même version, mais
vous pouvez aussi donner le vôtre.

```ts
import { Player } from "@nwasabi/js";
import { wasmUrl } from "@nwasabi/wasm";

const player = await Player.create(await fetch(wasmUrl));
player.attach(document.querySelector("canvas")!);
player.load(new Uint8Array(await file.arrayBuffer()), file.name);
```

Documentation, code source et suivi des problèmes : <https://github.com/Oignontom8283/nwasabi>.

## Licence

MIT ou Apache-2.0, au choix (voir `LICENSE-MIT` et `LICENSE-APACHE`).
