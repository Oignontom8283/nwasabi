// Enveloppe JavaScript du module WebAssembly de nwasabi (aucune dépendance).
//
// Le module ne parle que par pointeurs : on écrit une requête JSON dans sa mémoire, on appelle
// `nwasabi_request`, on relit la réponse. Les requêtes et les réponses sont celles du protocole de
// session (voir la commande `help`), les mêmes que celles de `nwasabi-cli session`.

export const SCREEN_WIDTH = 320;
export const SCREEN_HEIGHT = 240;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** Octets → base64 (par morceaux : `String.fromCharCode` ne supporte pas les très longs tableaux). */
export function toBase64(bytes) {
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(text);
}

/** base64 → octets. */
export function fromBase64(text) {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export class Nwasabi {
  /** @param {WebAssembly.Exports} exports */
  constructor(exports) {
    this.wasm = exports;
  }

  /**
   * Charge le module. `source` : `Response`, `ArrayBuffer`/`Uint8Array` ou `WebAssembly.Module`.
   * @returns {Promise<Nwasabi>}
   */
  static async load(source) {
    let instance;
    if (source instanceof WebAssembly.Module) {
      instance = await WebAssembly.instantiate(source, {});
    } else if (typeof Response !== "undefined" && source instanceof Response) {
      // La compilation « en continu » exige le type MIME `application/wasm`, que certains serveurs
      // (anciens `python -m http.server`…) ne donnent pas : dans ce cas on recharge les octets.
      const copy = source.clone();
      try {
        instance = (await WebAssembly.instantiateStreaming(source, {})).instance;
      } catch (error) {
        if (!(error instanceof TypeError)) throw error;
        instance = (await WebAssembly.instantiate(await copy.arrayBuffer(), {})).instance;
      }
    } else {
      instance = (await WebAssembly.instantiate(source, {})).instance;
    }
    return new Nwasabi(instance.exports);
  }

  /**
   * Envoie une requête et rend la réponse (objet JSON). Une réponse `{ok: false}` n'est pas une
   * exception : à vous de tester `ok`. Une panne du module (trap) lève bien une exception.
   */
  request(object) {
    const bytes = encoder.encode(JSON.stringify(object));
    const ptr = this.wasm.nwasabi_alloc(bytes.length);
    new Uint8Array(this.wasm.memory.buffer, ptr, bytes.length).set(bytes);
    const length = this.wasm.nwasabi_request(ptr, bytes.length);
    this.wasm.nwasabi_free(ptr, bytes.length);
    const reply = new Uint8Array(this.wasm.memory.buffer, this.wasm.nwasabi_response_ptr(), length);
    return JSON.parse(decoder.decode(reply));
  }

  /** Comme `request`, mais lève une exception si la réponse n'est pas `ok`. */
  call(object) {
    const reply = this.request(object);
    if (!reply.ok) throw new Error(reply.error);
    return reply;
  }

  /** Les pixels de l'écran (RGB565), en lecture seule ; `null` sans appareil. Valable jusqu'à la prochaine requête. */
  framebuffer() {
    const ptr = this.wasm.nwasabi_framebuffer_ptr();
    if (!ptr) return null;
    return new Uint16Array(this.wasm.memory.buffer, ptr, this.wasm.nwasabi_framebuffer_len());
  }

  /** Recopie l'écran dans une `ImageData` (RGBA) de 320×240. */
  drawTo(imageData) {
    const pixels = this.framebuffer();
    if (!pixels) return false;
    const out = imageData.data;
    for (let i = 0; i < pixels.length; i++) {
      const p = pixels[i];
      const r = (p >> 11) & 31;
      const g = (p >> 5) & 63;
      const b = p & 31;
      out[i * 4] = (r << 3) | (r >> 2);
      out[i * 4 + 1] = (g << 2) | (g >> 4);
      out[i * 4 + 2] = (b << 3) | (b >> 2);
      out[i * 4 + 3] = 255;
    }
    return true;
  }
}
