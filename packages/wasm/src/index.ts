/**
 * Adresse du module WebAssembly de nwasabi, à donner à `Nwasabi.load` ou `Player.create` de
 * `@nwasabi/js` (par exemple `fetch(wasmUrl)`). Les bundlers (Vite, webpack…) reconnaissent cette
 * écriture et copient le fichier dans le site construit.
 */
export const wasmUrl: URL = new URL("../nwasabi.wasm", import.meta.url);

export default wasmUrl;
