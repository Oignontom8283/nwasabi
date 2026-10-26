// Boîtes de dialogue simples (confirmation, saisie, choix de cases à cocher) : une file, affichée par DialogHost.

import { reactive } from "vue";

export interface Choice {
  id: string;
  label: string;
  hint?: string;
  checked?: boolean;
}

export interface DialogOptions {
  title?: string;
  accept?: string;
  /** Texte du bouton d'annulation (par défaut « Annuler »). */
  cancel?: string;
  danger?: boolean;
}

type Answer = boolean | string | string[] | null;

export interface Dialog extends DialogOptions {
  kind: "confirm" | "prompt" | "choices" | "alert";
  message: string;
  value?: string;
  choices?: Choice[];
  id: number;
  resolve: (answer: Answer) => void;
}

export const dialogState = reactive<{ current: Dialog | null }>({ current: null });
const queue: Dialog[] = [];
let counter = 0;

function next(): void {
  dialogState.current = queue.shift() ?? null;
}

function open<T extends Answer>(spec: Omit<Dialog, "resolve" | "id">): Promise<T> {
  return new Promise((resolve) => {
    queue.push({ ...spec, resolve: resolve as (answer: Answer) => void, id: ++counter });
    if (!dialogState.current) next();
  });
}

export function closeDialog(value: Answer): void {
  const current = dialogState.current;
  if (!current) return;
  current.resolve(value);
  next();
}

/** Demande de confirmation : `true` si acceptée. */
export const confirmDialog = (message: string, options: DialogOptions = {}) => open<boolean>({ kind: "confirm", message, ...options });

/** Saisie d'un texte : le texte, ou `null` si annulé. */
export const promptDialog = (message: string, value = "", options: DialogOptions = {}) => open<string | null>({ kind: "prompt", message, value, ...options });

/** Choix parmi des cases à cocher : la liste des identifiants cochés, ou `null`. */
export const choiceDialog = (message: string, choices: Choice[], options: DialogOptions = {}) => open<string[] | null>({ kind: "choices", message, choices, ...options });

/** Un message à lire. */
export const alertDialog = (message: string, options: DialogOptions = {}) => open<boolean>({ kind: "alert", message, ...options });
