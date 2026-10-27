// Conserver les fichiers de la calculatrice dans le navigateur (réglage facultatif, désactivé par défaut) : à chaque
// modification ils sont copiés (au format `.nfs`) dans le stockage du site, et remis dans chaque appareil neuf.
// Ils sont partagés par tout ce qui tourne sur ce site, dans ce navigateur ; l'utilisateur peut tout effacer.
// En les activant on demande aussi au navigateur de rendre le stockage « persistant » (il ne l'efface plus de lui-même).

import { reactive } from "vue";
import type { Emulator } from "./emulator.js";
import { clearRecent } from "./recent.js";
import { poll, remember } from "./util.js";

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
let stopPolling: () => void = () => {};

/** Donne à l'émulateur les fichiers à remettre dans chaque appareil neuf (il le fait lui-même, avant la première image). */
function offerKept(): void {
  emulator?.setKept(keep.enabled ? remember.get<Nfs | null>(KEY, null) : null);
}

async function save() {
  if (!keep.enabled || !emulator?.state.ready) return;
  const list = await emulator.quiet({ cmd: "fs_list" });
  if (!list.ok || list.generation === lastGeneration) return;
  const reply = await emulator.quiet({ cmd: "fs_export" });
  if (!reply.ok) return;
  lastGeneration = list.generation;
  keep.failed = !remember.set(KEY, reply.nfs);
  keep.stored = summary(reply.nfs);
}

/** Branche la conservation sur l'émulateur : à appeler une fois. */
export function attachPersistence(target: Emulator): void {
  emulator = target;
  offerKept();
  stopPolling();
  stopPolling = poll(save, POLL_MS);
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
  offerKept();
  await save();
}

export function disableKeeping() {
  keep.enabled = false;
  remember.set(ENABLED, false);
  offerKept();
}

/** Efface les fichiers conservés (pas ceux de l'appareil en cours). */
export async function clearKept() {
  remember.remove(KEY);
  void clearRecent();
  keep.stored = null;
  offerKept();
  // Ce qui est dans l'appareil en cours n'est pas recopié tout de suite : seule une modification le fera.
  lastGeneration = (await emulator?.quiet({ cmd: "fs_list" }))?.generation ?? -1;
}

export async function checkPersistent() {
  try {
    keep.persistent = (await navigator.storage?.persisted?.()) ?? null;
  } catch {
    keep.persistent = null;
  }
}
