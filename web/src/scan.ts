// Recherche de valeurs dans la mémoire, à la manière de Cheat Engine : une première recherche garde les adresses dont la
// valeur convient, puis chaque recherche suivante (valeur différente, « a changé », « a augmenté »…) retire celles qui ne
// conviennent plus, jusqu'à n'en garder que quelques-unes. Ce module ne touche ni à la page ni au module WebAssembly : la
// lecture de la mémoire lui est donnée (`MemoryReader`), ce qui permet de le tester seul.

/** Les types de valeur qu'on sait chercher. */
export type ScanType = "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "f32" | "f64";

export const SCAN_TYPES: readonly ScanType[] = ["u8", "i8", "u16", "i16", "u32", "i32", "f32", "f64"];

/** Taille en octets de chaque type. */
export const SIZE: Record<ScanType, number> = { u8: 1, i8: 1, u16: 2, i16: 2, u32: 4, i32: 4, f32: 4, f64: 8 };

const isFloat = (type: ScanType): boolean => type === "f32" || type === "f64";

/** Ce qu'on compare. `unknown` n'existe qu'à la première recherche : on garde toutes les adresses. */
export type Compare = "exact" | "greater" | "less" | "between" | "changed" | "unchanged" | "increased" | "decreased" | "unknown";

/** Les comparaisons qui ont besoin d'une valeur saisie, d'une ou de deux valeurs. */
export const NEEDS_VALUE: ReadonlySet<Compare> = new Set(["exact", "greater", "less", "between"]);
/** Les comparaisons à la valeur précédente, qui n'ont de sens qu'à partir de la deuxième recherche. */
export const NEEDS_PREVIOUS: ReadonlySet<Compare> = new Set(["changed", "unchanged", "increased", "decreased"]);

/** Lit la valeur de ce type à `offset` (little-endian, comme le processeur). */
export function decode(view: DataView, offset: number, type: ScanType): number {
  switch (type) {
    case "u8":
      return view.getUint8(offset);
    case "i8":
      return view.getInt8(offset);
    case "u16":
      return view.getUint16(offset, true);
    case "i16":
      return view.getInt16(offset, true);
    case "u32":
      return view.getUint32(offset, true);
    case "i32":
      return view.getInt32(offset, true);
    case "f32":
      return view.getFloat32(offset, true);
    case "f64":
      return view.getFloat64(offset, true);
  }
}

/** Les octets d'une valeur (le contraire de `decode`). */
export function encode(value: number, type: ScanType): Uint8Array {
  const bytes = new Uint8Array(SIZE[type]);
  const view = new DataView(bytes.buffer);
  switch (type) {
    case "u8":
      view.setUint8(0, value);
      break;
    case "i8":
      view.setInt8(0, value);
      break;
    case "u16":
      view.setUint16(0, value, true);
      break;
    case "i16":
      view.setInt16(0, value, true);
      break;
    case "u32":
      view.setUint32(0, value, true);
      break;
    case "i32":
      view.setInt32(0, value, true);
      break;
    case "f32":
      view.setFloat32(0, value, true);
      break;
    case "f64":
      view.setFloat64(0, value, true);
      break;
  }
  return bytes;
}

const RANGE: Record<Exclude<ScanType, "f32" | "f64">, [number, number]> = {
  u8: [0, 0xff],
  i8: [-0x80, 0x7f],
  u16: [0, 0xffff],
  i16: [-0x8000, 0x7fff],
  u32: [0, 0xffff_ffff],
  i32: [-0x8000_0000, 0x7fff_ffff],
};

/** Lit une valeur saisie (décimal, ou `0x…` pour un entier). `null` si elle n'est pas valable pour ce type. */
export function parseValue(text: string, type: ScanType): number | null {
  const t = text.trim().replace(",", ".");
  if (t === "") return null;
  if (isFloat(type)) {
    const value = Number(t);
    return Number.isFinite(value) ? value : null;
  }
  const value = /^[-+]?0x[0-9a-f]+$/i.test(t) ? Number.parseInt(t, 16) : /^[-+]?\d+$/.test(t) ? Number.parseInt(t, 10) : Number.NaN;
  const [low, high] = RANGE[type as keyof typeof RANGE];
  return Number.isInteger(value) && value >= low && value <= high ? value : null;
}

/** La valeur écrite pour l'affichage : les flottants sont arrondis à 7 chiffres significatifs. */
export function formatValue(value: number, type: ScanType): string {
  return isFloat(type) ? String(Number(value.toPrecision(type === "f32" ? 7 : 15))) : String(value);
}

/** Deux flottants sont égaux à un millionième près : `0.1` saisi retrouve le `0.1` d'un flottant 32 bits. */
const near = (a: number, b: number): boolean => Math.abs(a - b) <= Math.max(1e-6, Math.abs(b) * 1e-6);

export interface Criteria {
  compare: Compare;
  type: ScanType;
  /** Valeur saisie (`exact`, `greater`, `less`, début de `between`). */
  a?: number;
  /** Fin de `between`. */
  b?: number;
}

/** `value` convient-elle ? `previous` est la valeur de la recherche précédente (absente à la première). */
export function matches(criteria: Criteria, value: number, previous?: number): boolean {
  if (Number.isNaN(value)) return false;
  const float = isFloat(criteria.type);
  const { a = 0, b = 0 } = criteria;
  switch (criteria.compare) {
    case "unknown":
      return true;
    case "exact":
      return float ? near(value, a) : value === a;
    case "greater":
      return value > a;
    case "less":
      return value < a;
    case "between":
      return value >= Math.min(a, b) && value <= Math.max(a, b);
    case "changed":
      return previous !== undefined && (float ? !near(value, previous) : value !== previous);
    case "unchanged":
      return previous !== undefined && (float ? near(value, previous) : value === previous);
    case "increased":
      return previous !== undefined && value > previous && !(float && near(value, previous));
    case "decreased":
      return previous !== undefined && value < previous && !(float && near(value, previous));
  }
}

/** Une zone de mémoire à parcourir. */
export interface ScanRegion {
  base: number;
  size: number;
}

/** Lit `length` octets à `address` ; `null` si la lecture échoue. */
export type MemoryReader = (address: number, length: number) => Promise<Uint8Array | null>;

/** Octets lus d'un coup : une lecture par tranche, pas une par adresse. */
const CHUNK = 1 << 16;

/** Les adresses qui conviennent, avec leur valeur actuelle et celle de la recherche d'avant. */
export class Scan {
  addresses: number[] = [];
  values: number[] = [];
  previous: number[] = [];
  type: ScanType = "u32";
  /** Plus de cases qu'on ne veut en garder : la recherche s'est arrêtée. */
  truncated = false;

  constructor(private readonly limit = 4_000_000) {}

  get count(): number {
    return this.addresses.length;
  }

  /** Première recherche, dans ces zones. `aligned` : seulement aux adresses multiples de la taille du type. */
  async first(regions: readonly ScanRegion[], read: MemoryReader, criteria: Criteria, aligned = true): Promise<void> {
    this.addresses = [];
    this.values = [];
    this.previous = [];
    this.type = criteria.type;
    this.truncated = false;
    const size = SIZE[criteria.type];
    const step = aligned ? size : 1;
    for (const region of regions) {
      for (let start = 0; start < region.size; start += CHUNK) {
        const length = Math.min(CHUNK + size - 1, region.size - start);
        const bytes = await read(region.base + start, length);
        if (!bytes) continue;
        const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        const first = aligned ? (step - ((region.base + start) % step)) % step : 0;
        for (let offset = first; offset < Math.min(CHUNK, length) && offset + size <= bytes.length; offset += step) {
          const value = decode(view, offset, criteria.type);
          if (!matches(criteria, value)) continue;
          if (this.addresses.length >= this.limit) {
            this.truncated = true;
            return;
          }
          this.addresses.push(region.base + start + offset);
          this.values.push(value);
        }
      }
    }
  }

  /** Recherche suivante : relit les adresses gardées et ne garde que celles dont la nouvelle valeur convient. */
  async next(read: MemoryReader, criteria: Criteria): Promise<void> {
    const size = SIZE[this.type];
    const addresses: number[] = [];
    const values: number[] = [];
    const previous: number[] = [];
    let chunkStart = -1;
    let view: DataView | null = null;
    let length = 0;
    for (let i = 0; i < this.addresses.length; i++) {
      const address = this.addresses[i] as number;
      if (view === null || address < chunkStart || address + size > chunkStart + length) {
        chunkStart = address;
        const wanted = Math.min(CHUNK, (this.addresses[this.addresses.length - 1] as number) + size - address);
        const bytes = await read(address, wanted);
        view = bytes ? new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength) : null;
        length = bytes?.length ?? 0;
        if (!view) continue;
      }
      if (address + size > chunkStart + length) continue;
      const value = decode(view, address - chunkStart, this.type);
      const before = this.values[i] as number;
      if (!matches({ ...criteria, type: this.type }, value, before)) continue;
      addresses.push(address);
      values.push(value);
      previous.push(before);
    }
    this.addresses = addresses;
    this.values = values;
    this.previous = previous;
  }
}
