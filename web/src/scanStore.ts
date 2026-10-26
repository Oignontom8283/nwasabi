// L'état de la recherche de valeurs, hors des composants : on le retrouve en revenant sur l'onglet Mémoire, et les trois
// parties de l'interface (réglages, résultats, liste) le partagent sans se redessiner les unes les autres.

import { reactive, ref, shallowRef } from "vue";
import type { Emulator } from "./emulator.js";
import { Scan, SIZE, decode, encode } from "./scan.js";
import type { MemoryReader, ScanType } from "./scan.js";
import { hex, hexToBytes } from "./util.js";

export interface Watched {
  id: number;
  description: string;
  address: number;
  type: ScanType;
  /** Valeur lue à la dernière relève, ou `null` si la lecture a échoué. */
  current: number | null;
  /** Valeur maintenue en permanence (« maintenir »), ou `null`. */
  frozen: number | null;
  /** Texte en cours de saisie dans le champ de valeur, ou `null` : tant qu'on saisit, les relevés ne le remplacent pas. */
  draft: string | null;
}

export const scan = shallowRef(new Scan());
export const store = reactive({
  started: false,
  busy: false,
  /** Augmente à chaque recherche : les résultats se recalculent. */
  revision: 0,
  watched: [] as Watched[],
  nextId: 1,
});
/** Les erreurs de la liste (valeur saisie refusée, écriture impossible). */
export const listMessage = ref("");

/** Lecture et écriture de la mémoire de l'émulateur, pour la recherche et la liste. */
export function memoryAccess(emulator: Emulator) {
  const read: MemoryReader = async (address, length) => {
    const reply = await emulator.quiet({ cmd: "memory_read", address, length });
    return reply.ok ? hexToBytes(String(reply.hex)) : null;
  };

  async function readValue(address: number, type: ScanType): Promise<number | null> {
    const bytes = await read(address, SIZE[type]);
    return bytes && bytes.length === SIZE[type] ? decode(new DataView(bytes.buffer, bytes.byteOffset), 0, type) : null;
  }

  async function writeValue(address: number, type: ScanType, value: number): Promise<boolean> {
    const hexText = Array.from(encode(value, type), (b) => hex(b, 2)).join("");
    return (await emulator.quiet({ cmd: "memory_write", address, hex: hexText })).ok;
  }

  return { read, readValue, writeValue };
}

export function addWatched(address: number, type: ScanType, value: number | null): void {
  if (store.watched.some((w) => w.address === address && w.type === type)) return;
  store.watched.push({ id: store.nextId++, description: "", address, type, current: value, frozen: null, draft: null });
}
