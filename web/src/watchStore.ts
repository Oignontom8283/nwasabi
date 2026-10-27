// Les points d'observation vus par l'interface : ceux que le cœur a posés (il en est la seule source) et, pour chacun, les
// accès relevés, regroupés par instruction qui les fait (« quelle instruction écrit ici ? »).

import { reactive } from "vue";
import type { Emulator } from "./emulator.js";

export type WatchKind = "read" | "write" | "readwrite";

export interface Watchpoint {
  id: number;
  address: number;
  length: number;
  kind: WatchKind;
  stop: boolean;
}

/** Les accès d'une même instruction à un même point d'observation. */
export interface HitRow {
  key: string;
  id: number;
  /** Adresse de l'instruction qui fait l'accès. */
  pc: number;
  write: boolean;
  /** Dernière adresse touchée, sa largeur et la dernière valeur lue ou écrite. */
  address: number;
  width: number;
  value: number;
  count: number;
}

const MAX_ROWS = 200;

export const watching = reactive({
  list: [] as Watchpoint[],
  rows: [] as HitRow[],
});

/** Relit la liste des points posés dans le cœur. */
export async function refreshWatches(emulator: Emulator): Promise<void> {
  const reply = await emulator.quiet({ cmd: "watch_list" });
  const list = reply.ok ? (reply.watches as Watchpoint[]) : [];
  if (JSON.stringify(list) !== JSON.stringify(watching.list)) watching.list = list;
  if (list.length === 0 && watching.rows.length > 0) watching.rows = [];
}

/** Relève les accès observés depuis la dernière fois et les ajoute aux lignes. */
export async function drainHits(emulator: Emulator): Promise<void> {
  const reply = await emulator.quiet({ cmd: "watch_hits" });
  if (!reply.ok) return;
  const hits = reply.hits as Array<{ id: number; address: number; width: number; write: boolean; value: number; pc: number }>;
  if (hits.length === 0) return;
  const byKey = new Map(watching.rows.map((row) => [row.key, row]));
  for (const hit of hits) {
    const key = `${hit.id}:${hit.pc}:${hit.write}`;
    const row = byKey.get(key);
    if (row) {
      row.count++;
      row.address = hit.address;
      row.width = hit.width;
      row.value = hit.value;
    } else {
      const created = { key, id: hit.id, pc: hit.pc, write: hit.write, address: hit.address, width: hit.width, value: hit.value, count: 1 };
      watching.rows.push(created);
      byKey.set(key, created);
    }
  }
  watching.rows.sort((a, b) => b.count - a.count);
  if (watching.rows.length > MAX_ROWS) watching.rows.length = MAX_ROWS;
}

/** Pose un point d'observation ; rend le message d'erreur du cœur, ou `null`. */
export async function addWatch(emulator: Emulator, address: number, length: number, kind: WatchKind, stop: boolean): Promise<string | null> {
  const reply = await emulator.quiet({ cmd: "watch", address, length, kind, stop });
  if (!reply.ok) return String(reply.error);
  await refreshWatches(emulator);
  return null;
}

export async function removeWatch(emulator: Emulator, id: number): Promise<void> {
  await emulator.quiet({ cmd: "unwatch", id });
  watching.rows = watching.rows.filter((row) => row.id !== id);
  await refreshWatches(emulator);
}

export const clearHits = (): void => void (watching.rows = []);
