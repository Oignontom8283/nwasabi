// Les dernières applications ouvertes, gardées dans le navigateur (IndexedDB) avec la même autorisation que les fichiers de la
// calculatrice : elles sont proposées dans le menu pour les rouvrir sans retourner chercher le fichier.

import { reactive } from "vue";
import { idbDelete, idbGet, idbSet } from "./idb.js";

const KEY = "recent-apps";
const MAX = 6;

interface Stored {
  name: string;
  bytes: Uint8Array;
  at: number;
}

/** Ce que le menu affiche : le nom et la date d'ouverture. */
export const recent = reactive({ items: [] as Array<{ name: string; at: number }> });

const summary = (list: Stored[]) => list.map(({ name, at }) => ({ name, at }));

/** Relit la liste conservée (au démarrage). */
export async function refreshRecent(): Promise<void> {
  recent.items = summary((await idbGet<Stored[]>(KEY)) ?? []);
}

/** Retient une application ouverte (la plus récente en premier ; un même nom n'apparaît qu'une fois). */
export async function rememberApp(name: string, bytes: Uint8Array): Promise<void> {
  const list = ((await idbGet<Stored[]>(KEY)) ?? []).filter((item) => item.name !== name);
  list.unshift({ name, bytes, at: Date.now() });
  list.length = Math.min(list.length, MAX);
  if (await idbSet(KEY, list)) recent.items = summary(list);
}

/** Le contenu d'une application récente, ou `null`. */
export async function recentBytes(name: string): Promise<Uint8Array | null> {
  return ((await idbGet<Stored[]>(KEY)) ?? []).find((item) => item.name === name)?.bytes ?? null;
}

export async function clearRecent(): Promise<void> {
  await idbDelete(KEY);
  recent.items = [];
}
