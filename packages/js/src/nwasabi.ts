// Enveloppe du module WebAssembly de nwasabi (aucune dépendance).
//
// Le module ne parle que par pointeurs : on écrit une requête JSON dans sa mémoire, on appelle
// `nwasabi_request`, on relit la réponse. Les requêtes et les réponses sont celles du protocole de
// session (voir la commande `help`), les mêmes que celles de `nwasabi-cli session`.

export const SCREEN_WIDTH = 320;
export const SCREEN_HEIGHT = 240;

/** Une requête du protocole : `cmd` plus les champs propres à la commande. */
export interface Request {
  cmd: string;
  [field: string]: unknown;
}

/** Une réponse du protocole : `ok`, puis `error` ou les champs propres à la commande. */
export interface Reply {
  ok: boolean;
  error?: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [field: string]: any;
}

/** Ce que `Nwasabi.load` accepte : une réponse HTTP, des octets ou un module déjà compilé. */
export type WasmSource = Response | ArrayBuffer | Uint8Array | WebAssembly.Module;

/** Les fonctions exportées par le module (voir `src/wasm.rs` dans le dépôt de nwasabi). */
interface Exports {
  memory: WebAssembly.Memory;
  nwasabi_alloc(length: number): number;
  nwasabi_free(pointer: number, length: number): void;
  nwasabi_request(pointer: number, length: number): number;
  nwasabi_response_ptr(): number;
  nwasabi_framebuffer_ptr(): number;
  nwasabi_framebuffer_len(): number;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** Octets → base64 (par morceaux : `String.fromCharCode` ne supporte pas les très longs tableaux). */
export function toBase64(bytes: Uint8Array): string {
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(text);
}

/** base64 → octets. */
export function fromBase64(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export class Nwasabi {
  private readonly wasm: Exports;

  private constructor(exports: WebAssembly.Exports) {
    this.wasm = exports as unknown as Exports;
  }

  /**
   * Charge le module. C'est à l'appelant de fournir le `.wasm` (ce paquet n'en embarque pas) :
   * une `Response` (`fetch`), des octets ou un `WebAssembly.Module`.
   */
  static async load(source: WasmSource): Promise<Nwasabi> {
    let instance: WebAssembly.Instance;
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
      instance = (await WebAssembly.instantiate(source as BufferSource, {})).instance;
    }
    return new Nwasabi(instance.exports);
  }

  /**
   * Envoie une requête et rend la réponse. Une réponse `{ok: false}` n'est pas une exception : à
   * vous de tester `ok`. Une panne du module (trap) lève bien une exception.
   */
  request(request: Request): Reply {
    const bytes = encoder.encode(JSON.stringify(request));
    const pointer = this.wasm.nwasabi_alloc(bytes.length);
    new Uint8Array(this.wasm.memory.buffer, pointer, bytes.length).set(bytes);
    const length = this.wasm.nwasabi_request(pointer, bytes.length);
    this.wasm.nwasabi_free(pointer, bytes.length);
    const reply = new Uint8Array(this.wasm.memory.buffer, this.wasm.nwasabi_response_ptr(), length);
    return JSON.parse(decoder.decode(reply)) as Reply;
  }

  /** Comme `request`, mais lève une exception si la réponse n'est pas `ok`. */
  call(request: Request): Reply {
    const reply = this.request(request);
    if (!reply.ok) throw new Error(reply.error);
    return reply;
  }

  /** Les pixels de l'écran (RGB565), en lecture seule ; `null` sans appareil. Valable jusqu'à la prochaine requête. */
  framebuffer(): Uint16Array | null {
    const pointer = this.wasm.nwasabi_framebuffer_ptr();
    if (!pointer) return null;
    return new Uint16Array(this.wasm.memory.buffer, pointer, this.wasm.nwasabi_framebuffer_len());
  }

  /** Recopie l'écran dans une `ImageData` (RGBA) de 320×240 ; faux s'il n'y a pas d'appareil. */
  drawTo(image: ImageData): boolean {
    const pixels = this.framebuffer();
    if (!pixels) return false;
    const out = image.data;
    for (let i = 0; i < pixels.length; i++) {
      const p = pixels[i] ?? 0;
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
