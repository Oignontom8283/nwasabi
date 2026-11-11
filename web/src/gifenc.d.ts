// Types de la partie de `gifenc` qu'utilise l'enregistreur de GIF (la bibliothèque n'en fournit pas).
declare module "gifenc" {
  export type Palette = number[][];
  export interface FrameOptions {
    palette?: Palette;
    /** Durée de l'image, en millisecondes. */
    delay?: number;
    /** Nombre de répétitions : 0 pour toujours, -1 pour une seule fois. */
    repeat?: number;
  }
  export interface Encoder {
    writeFrame(index: Uint8Array, width: number, height: number, options?: FrameOptions): void;
    finish(): void;
    bytes(): Uint8Array;
  }
  export function GIFEncoder(options?: { initialCapacity?: number; auto?: boolean }): Encoder;
  export function quantize(rgba: Uint8Array | Uint8ClampedArray, maxColors: number, options?: { format?: "rgb565" | "rgb444" | "rgba4444" }): Palette;
  export function applyPalette(rgba: Uint8Array | Uint8ClampedArray, palette: Palette, format?: "rgb565" | "rgb444" | "rgba4444"): Uint8Array;
}
