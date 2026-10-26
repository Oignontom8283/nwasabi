// Boîtes de dialogue simples (confirmation, saisie, choix de cases à cocher) : une file, affichée par DialogHost.

import { reactive } from "vue";

export const dialogState = reactive({ current: null });
const queue = [];

function next() {
  dialogState.current = queue.shift() ?? null;
}

function open(spec) {
  return new Promise((resolve) => {
    queue.push({ ...spec, resolve });
    if (!dialogState.current) next();
  });
}

export function closeDialog(value) {
  const current = dialogState.current;
  if (!current) return;
  current.resolve(value);
  next();
}

/** Demande de confirmation : `true` si acceptée. */
export const confirmDialog = (message, options = {}) => open({ kind: "confirm", message, ...options });

/** Saisie d'un texte : le texte, ou `null` si annulé. */
export const promptDialog = (message, value = "", options = {}) => open({ kind: "prompt", message, value, ...options });

/** Choix parmi des cases à cocher `{ id, label, hint, checked }` : la liste des identifiants cochés, ou `null`. */
export const choiceDialog = (message, choices, options = {}) => open({ kind: "choices", message, choices, ...options });

/** Un message à lire. */
export const alertDialog = (message, options = {}) => open({ kind: "alert", message, ...options });
