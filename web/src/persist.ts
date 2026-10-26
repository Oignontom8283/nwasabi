// Conserver les fichiers de la calculatrice dans le navigateur (réglage facultatif, désactivé par défaut) : à chaque
// modification ils sont copiés (au format `.nfs`) dans le stockage du site, et remis dans chaque appareil neuf.
// Ils sont partagés par tout ce qui tourne sur ce site, dans ce navigateur ; l'utilisateur peut tout effacer.
// En les activant on demande aussi au navigateur de rendre le stockage « persistant » (il ne l'efface plus de lui-même).

import { reactive } from "vue";
import type { Emulator } from "./emulator.js";
import { remember } from "./util.js";

/** Un document `.nfs` : seul ce que ce module lit est décrit. */
interface Nfs {
  files?: unknown[];
}

const KEY = "nwasabi.files";
const ENABLED = "nwasabi.keepFiles";
const POLL_MS = 1000;

export const keep = reactive({
  enabled: remember.get<boolean | null>(ENABLED, null) === true,
  /** Le navigateur a-t-il accordé le stockage persistant ? `null` : pas demandé ou pas pris en charge. */
  persistent: null as boolean | null,
  /** Ce qui est conservé : nombre de fichiers et taille du stockage utilisée, ou `null`. */
  stored: summary(remember.get<Nfs | null>(KEY, null)),
  /** Le dernier enregistrement a échoué (stockage plein ou bloqué) ? */
  failed: false,
});

function summary(doc: Nfs | null): { files: number; bytes: number } | null {
  if (!doc?.files) return null;
  return { files: doc.files.length, bytes: JSON.stringify(doc).length };
}

let emulator: Emulator | null = null;
let lastGeneration = -1;
let timer: ReturnType<typeof setInterval> | undefined;

function restoreInto() {
  const doc = remember.get<Nfs | null>(KEY, null);
  if (!keep.enabled || !doc || !emulator) return;
  const reply = emulator.quiet({ cmd: "fs_import", nfs: doc, replace: true });
  if (reply.ok) lastGeneration = emulator.quiet({ cmd: "fs_list" }).generation ?? -1;
}

function save() {
  if (!keep.enabled || !emulator?.state.loaded) return;
  const list = emulator.quiet({ cmd: "fs_list" });
  if (!list.ok || list.generation === lastGeneration) return;
  const reply = emulator.quiet({ cmd: "fs_export" });
  if (!reply.ok) return;
  lastGeneration = list.generation;
  keep.failed = !remember.set(KEY, reply.nfs);
  keep.stored = summary(reply.nfs);
}

/** Branche la conservation sur l'émulateur : à appeler une fois. */
export function attachPersistence(target: Emulator): void {
  emulator = target;
  emulator.onDevice(restoreInto);
  clearInterval(timer);
  timer = setInterval(save, POLL_MS);
}

/** Le visiteur n'a pas encore répondu : le site pose la question à la première visite. */
export const needsAnswer = (): boolean => remember.get<boolean | null>(ENABLED, null) === null;

export async function enableKeeping() {
  try {
    keep.persistent = (await navigator.storage?.persist?.()) ?? null;
  } catch {
    keep.persistent = null;
  }
  keep.enabled = true;
  remember.set(ENABLED, true);
  lastGeneration = -1;
  save();
}

export function disableKeeping() {
  keep.enabled = false;
  remember.set(ENABLED, false);
}

/** Efface les fichiers conservés (pas ceux de l'appareil en cours). */
export function clearKept() {
  remember.remove(KEY);
  keep.stored = null;
  // Ce qui est dans l'appareil en cours n'est pas recopié tout de suite : seule une modification le fera.
  lastGeneration = emulator?.quiet({ cmd: "fs_list" }).generation ?? -1;
}

export async function checkPersistent() {
  try {
    keep.persistent = (await navigator.storage?.persisted?.()) ?? null;
  } catch {
    keep.persistent = null;
  }
}
