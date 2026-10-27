// Sauvegarde rapide de l'état de l'émulation dans le navigateur (un emplacement), sans passer par un fichier `.nds`.
// Ctrl + S enregistre, Ctrl + O recharge. L'état est celui de `Player.dump` : application, mémoire, fichiers, processeur.

import { reactive } from "vue";
import type { Emulator } from "./emulator.js";
import { idbGet, idbSet } from "./idb.js";
import { notify } from "./ui.js";

const KEY = "quick-state";
const META = "quick-state-meta";

export const quick = reactive<{ savedAt: number | null; appName: string }>({ savedAt: null, appName: "" });

/** Relit ce qui est conservé (date et nom de l'application), sans charger l'état lui-même. */
export async function refreshQuick(): Promise<void> {
  const meta = await idbGet<{ savedAt: number; appName: string }>(META);
  quick.savedAt = meta?.savedAt ?? null;
  quick.appName = meta?.appName ?? "";
}

/** Enregistre l'état. Le lecteur refuse d'enregistrer en marche : on le met en pause un instant, puis on le relance. */
export async function quickSave(emulator: Emulator, messages: { saved: string; failed: string }): Promise<boolean> {
  const { state } = emulator;
  if (!state.loaded) return false;
  const wasRunning = state.running;
  if (wasRunning) await emulator.stop();
  const document = await emulator.dump(["app", "memory", "fs", "state"]);
  if (wasRunning) await emulator.start();
  const saved = !!document && (await idbSet(KEY, document));
  if (saved) {
    const meta = { savedAt: Date.now(), appName: state.appName };
    await idbSet(META, meta);
    quick.savedAt = meta.savedAt;
    quick.appName = meta.appName;
  }
  notify(saved ? messages.saved : messages.failed);
  return saved;
}

/** Recharge l'état enregistré (l'application qu'il contient remplace celle qui tourne) puis reprend la simulation. */
export async function quickLoad(emulator: Emulator, messages: { loaded: string; none: string; failed: string }): Promise<boolean> {
  const document = await idbGet<unknown>(KEY);
  if (!document) {
    notify(messages.none);
    return false;
  }
  const ok = (await emulator.restore(document)) === true;
  if (ok) await emulator.start();
  notify(ok ? messages.loaded : messages.failed);
  return ok;
}
