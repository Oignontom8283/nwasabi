// Enregistrement de l'écran en GIF animé, sans bibliothèque : palette de 256 couleurs choisie sur les premières images
// (l'écran d'une calculatrice a peu de couleurs), compression LZW de chaque image au fil de l'eau. Une image identique à la
// précédente n'est pas répétée : on rallonge la durée de la précédente.

const MAX_COLORS = 256;

/** Compresse des indices de palette (8 bits) en LZW de GIF ; rend les octets, déjà découpés en blocs de 255 octets. */
export function lzw(indices: Uint8Array, minCodeSize = 8): Uint8Array {
  const clear = 1 << minCodeSize;
  const end = clear + 1;
  const out: number[] = [];
  let bits = 0;
  let acc = 0;
  let size = minCodeSize + 1;
  let next = end + 1;
  let dictionary = new Map<number, number>();
  const emit = (code: number) => {
    acc |= code << bits;
    bits += size;
    while (bits >= 8) {
      out.push(acc & 0xff);
      acc >>>= 8;
      bits -= 8;
    }
  };
  const reset = () => {
    dictionary = new Map();
    size = minCodeSize + 1;
    next = end + 1;
  };
  emit(clear);
  let prefix = indices[0] ?? 0;
  for (let i = 1; i < indices.length; i++) {
    const k = indices[i] as number;
    const key = (prefix << 8) | k;
    const found = dictionary.get(key);
    if (found !== undefined) {
      prefix = found;
      continue;
    }
    emit(prefix);
    if (next < 4096) {
      dictionary.set(key, next++);
      if (next - 1 === 1 << size && size < 12) size++;
    } else {
      emit(clear);
      reset();
    }
    prefix = k;
  }
  emit(prefix);
  emit(end);
  if (bits > 0) out.push(acc & 0xff);
  // Blocs de données : une longueur (1 à 255) puis les octets, terminés par un bloc vide.
  const blocks: number[] = [];
  for (let i = 0; i < out.length; i += 255) {
    const chunk = out.slice(i, i + 255);
    blocks.push(chunk.length, ...chunk);
  }
  blocks.push(0);
  return Uint8Array.from(blocks);
}

/** Une couleur sur 15 bits (5 bits par composante) : la clé de la palette. */
const key15 = (r: number, g: number, b: number): number => ((r >> 3) << 10) | ((g >> 3) << 5) | (b >> 3);

export class GifRecorder {
  private readonly parts: Uint8Array[] = [];
  private palette: Uint8Array | null = null; // 256 × RGB
  private lookup = new Int16Array(32768).fill(-1);
  private pending: { indices: Uint8Array; delay: number } | null = null;
  private lastAt = 0;
  frames = 0;

  constructor(
    private readonly width: number,
    private readonly height: number,
    /** Durée minimale entre deux images retenues, en millisecondes (le GIF est plus léger qu'à 60 images par seconde). */
    private readonly interval = 40,
  ) {}

  /** Palette : les couleurs les plus fréquentes de cette image (au plus 256). */
  private choosePalette(rgba: Uint8ClampedArray): void {
    const histogram = new Map<number, number>();
    for (let i = 0; i < rgba.length; i += 4) {
      const k = key15(rgba[i] as number, rgba[i + 1] as number, rgba[i + 2] as number);
      histogram.set(k, (histogram.get(k) ?? 0) + 1);
    }
    const top = [...histogram.entries()].sort((a, b) => b[1] - a[1]).slice(0, MAX_COLORS);
    const palette = new Uint8Array(MAX_COLORS * 3);
    top.forEach(([k], i) => {
      palette[i * 3] = ((k >> 10) & 31) << 3 | 4;
      palette[i * 3 + 1] = ((k >> 5) & 31) << 3 | 4;
      palette[i * 3 + 2] = (k & 31) << 3 | 4;
      this.lookup[k] = i;
    });
    this.palette = palette;
  }

  private indexOf(r: number, g: number, b: number): number {
    const k = key15(r, g, b);
    const known = this.lookup[k] as number;
    if (known >= 0) return known;
    // Couleur absente de la palette : la plus proche (distance euclidienne), retenue pour les fois suivantes.
    const palette = this.palette as Uint8Array;
    let best = 0;
    let bestDistance = Infinity;
    for (let i = 0; i < MAX_COLORS; i++) {
      const dr = (palette[i * 3] as number) - r;
      const dg = (palette[i * 3 + 1] as number) - g;
      const db = (palette[i * 3 + 2] as number) - b;
      const distance = dr * dr + dg * dg + db * db;
      if (distance < bestDistance) {
        bestDistance = distance;
        best = i;
      }
    }
    this.lookup[k] = best;
    return best;
  }

  /** Ajoute une image (RGBA, `width × height`), vue à l'instant `now` (millisecondes). Les images trop rapprochées sont ignorées. */
  add(rgba: Uint8ClampedArray, now: number): void {
    if (this.frames > 0 && now - this.lastAt < this.interval) return;
    if (!this.palette) this.choosePalette(rgba);
    const indices = new Uint8Array(this.width * this.height);
    for (let i = 0, p = 0; p < indices.length; p++, i += 4) indices[p] = this.indexOf(rgba[i] as number, rgba[i + 1] as number, rgba[i + 2] as number);
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
    if (!frame) return;
    const centis = Math.max(2, Math.round((frame.delay + extraDelay) / 10));
    const w = this.width;
    const h = this.height;
    const head = Uint8Array.of(
      0x21, 0xf9, 4, 0, centis & 0xff, centis >> 8, 0, 0, // contrôle de l'image : durée
      0x2c, 0, 0, 0, 0, w & 0xff, w >> 8, h & 0xff, h >> 8, 0, // descripteur : toute la surface, palette globale
      8, // taille minimale des codes LZW
    );
    this.parts.push(head, lzw(frame.indices));
    this.pending = null;
  }

  /** Termine l'enregistrement et rend le fichier GIF. */
  finish(): Uint8Array {
    if (this.pending) this.flush(this.interval);
    const w = this.width;
    const h = this.height;
    const header = Uint8Array.of(
      0x47, 0x49, 0x46, 0x38, 0x39, 0x61, // GIF89a
      w & 0xff, w >> 8, h & 0xff, h >> 8,
      0xf7, 0, 0, // palette globale de 256 couleurs
    );
    const loop = Uint8Array.of(0x21, 0xff, 11, ...new TextEncoder().encode("NETSCAPE2.0"), 3, 1, 0, 0, 0);
    const trailer = Uint8Array.of(0x3b);
    const palette = this.palette ?? new Uint8Array(MAX_COLORS * 3);
    const all = [header, palette, loop, ...this.parts, trailer];
    const out = new Uint8Array(all.reduce((n, p) => n + p.length, 0));
    let at = 0;
    for (const part of all) {
      out.set(part, at);
      at += part.length;
    }
    return out;
  }
}
