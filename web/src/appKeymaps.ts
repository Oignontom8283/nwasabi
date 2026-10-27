// Un mapping de touches propre à une application : retenu dans le navigateur sous le nom de l'application, il remplace le
// mapping général quand cette application est chargée.

import { reactive } from "vue";
import { KeyMap } from "@nwasabi/js";
import { remember } from "./util.js";

const PREFIX = "nwasabi.keymap.";

/** Applications qui ont leur mapping (pour que l'interface se mette à jour quand on en retient ou oublie un). */
export const appKeymaps = reactive({ version: 0 });

export function hasAppKeymap(app: string): boolean {
  void appKeymaps.version;
  return app !== "" && remember.get<unknown>(PREFIX + app, null) !== null;
}

/** Le mapping retenu pour cette application, ou `null` (absent ou illisible). */
export function loadAppKeymap(app: string): KeyMap | null {
  const stored = remember.get<unknown>(PREFIX + app, null);
  if (stored === null) return null;
  try {
    return KeyMap.parse(stored);
  } catch {
    return null;
  }
}

export function saveAppKeymap(app: string, map: KeyMap): boolean {
  const done = remember.set(PREFIX + app, map.toNmk());
  appKeymaps.version++;
  return done;
}

export function forgetAppKeymap(app: string): void {
  remember.remove(PREFIX + app);
  appKeymaps.version++;
}
