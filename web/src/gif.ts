// Enregistrement de l'écran en GIF animé : la palette de 256 couleurs est choisie sur la première image (l'écran d'une calculatrice a peu
// de couleurs) et la compression est faite par `gifenc`. Une image identique à la précédente n'est pas répétée : on rallonge la durée
// de la précédente.

import { applyPalette, GIFEncoder, quantize } from "gifenc";
import type { Encoder, Palette } from "gifenc";

const MAX_COLORS = 256;
/** Durée minimale d'une image, en millisecondes (un GIF compte en centièmes de seconde). */
const MIN_DELAY = 20;

export class GifRecorder {
  private readonly encoder: Encoder = GIFEncoder();
  private palette: Palette | null = null;
  private pending: { indices: Uint8Array; delay: number } | null = null;
  private lastAt = 0;
  frames = 0;

  constructor(
    private readonly width: number,
    private readonly height: number,
    /** Durée minimale entre deux images retenues, en millisecondes (le GIF est plus léger qu'à 60 images par seconde). */
    private readonly interval = 40,
  ) {}

  /** Ajoute une image (RGBA, `width × height`), vue à l'instant `now` (millisecondes). Les images trop rapprochées sont ignorées. */
  add(rgba: Uint8ClampedArray, now: number): void {
    if (this.frames > 0 && now - this.lastAt < this.interval) return;
    if (!this.palette) this.palette = quantize(rgba, MAX_COLORS);
    const indices = applyPalette(rgba, this.palette);
    const elapsed = this.frames === 0 ? this.interval : now - this.lastAt;
    this.lastAt = now;
    this.frames++;
    if (this.pending) {
      const same = this.pending.indices.every((v, i) => v === indices[i]);
      if (same) {
        this.pending.delay += elapsed;
        return;
      }
      this.flush(elapsed);
    }
    this.pending = { indices, delay: 0 };
  }

  /** Écrit l'image en attente, avec la durée pendant laquelle elle est restée à l'écran. */
  private flush(extraDelay: number): void {
    const frame = this.pending;
    if (!frame || !this.palette) return;
    const delay = Math.max(MIN_DELAY, frame.delay + extraDelay);
    // La première image porte la palette (globale) et la boucle infinie ; les suivantes l'utilisent.
    const first = this.encoder.bytes().length === 0;
    this.encoder.writeFrame(frame.indices, this.width, this.height, first ? { palette: this.palette, delay, repeat: 0 } : { delay });
    this.pending = null;
  }

  /** Termine l'enregistrement et rend le fichier GIF. */
  finish(): Uint8Array {
    if (this.pending) this.flush(this.interval);
    this.encoder.finish();
    return this.encoder.bytes();
  }
}
