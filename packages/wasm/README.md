# @nwasabi/wasm

Le module WebAssembly de [nwasabi](https://github.com/Oignontom8283/nwasabi), un émulateur
d'applications externes NumWorks (`.nwa`). Il s'utilise avec
[`@nwasabi/js`](https://www.npmjs.com/package/@nwasabi/js), qui sait lui parler.

```ts
import { Player } from "@nwasabi/js";
import { wasmUrl } from "@nwasabi/wasm";

const player = await Player.create(await fetch(wasmUrl));
```

`wasmUrl` est l'adresse du fichier `nwasabi.wasm`, que les bundlers (Vite, webpack…) copient dans le
site construit. Les deux paquets sont publiés ensemble, à la même version.

Documentation, code source et suivi des problèmes : <https://github.com/Oignontom8283/nwasabi>.

## Licence

MIT ou Apache-2.0, au choix (voir `LICENSE-MIT` et `LICENSE-APACHE`).
