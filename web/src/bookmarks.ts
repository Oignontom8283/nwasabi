// Les signets de la mémoire : des zones nommées, retenues par application (par son nom) dans le navigateur.

import { reactive } from "vue";
import type { ScanType } from "./scan.js";
import { hex0x, remember } from "./util.js";

export interface Bookmark {
  id: number;
  name: string;
  address: number;
  length: number;
  /** Façon de lire et d'écrire la valeur de la zone (le début de la zone, si elle est plus longue). */
  type?: ScanType;
}

/** Le type par défaut d'une zone : celui qui tient dans sa longueur (1, 2 ou 4 octets). */
export const defaultType = (length: number): ScanType => (length >= 4 ? "u32" : length >= 2 ? "u16" : "u8");
/** Le type d'un signet, qu'il l'ait choisi ou non. */
export const typeOf = (b: Bookmark): ScanType => b.type ?? defaultType(b.length);

const PREFIX = "nwasabi.bookmarks.";

export const bookmarks = reactive({ app: "", list: [] as Bookmark[] });

/** Charge les signets de cette application. */
export function loadBookmarks(app: string): void {
  bookmarks.app = app;
  bookmarks.list = app ? remember.get<Bookmark[]>(PREFIX + app, []) : [];
}

function save(): void {
  if (!bookmarks.app) return;
  if (bookmarks.list.length === 0) remember.remove(PREFIX + bookmarks.app);
  else remember.set(PREFIX + bookmarks.app, bookmarks.list);
}

/** Ajoute un signet (le nom par défaut est l'adresse). Rend `false` s'il n'y a pas d'application. */
export function addBookmark(address: number, length: number, name = ""): boolean {
  if (!bookmarks.app) return false;
  const id = bookmarks.list.reduce((max, b) => Math.max(max, b.id), 0) + 1;
  bookmarks.list.push({ id, name: name || hex0x(address), address, length });
  save();
  return true;
}

export function renameBookmark(id: number, name: string): void {
  const found = bookmarks.list.find((b) => b.id === id);
  if (found) found.name = name;
  save();
}

export function setBookmarkType(id: number, type: ScanType): void {
  const found = bookmarks.list.find((b) => b.id === id);
  if (found) found.type = type;
  save();
}

export function removeBookmark(id: number): void {
  bookmarks.list = bookmarks.list.filter((b) => b.id !== id);
  save();
}
