// Langues de l'interface : un fichier par langue dans `locales/`.
// Choix de la langue, dans l'ordre : `?lang=fr` dans l'adresse, le dernier choix de l'utilisateur, la langue du
// navigateur, sinon l'anglais.

import { createI18n } from "vue-i18n";
import en from "./locales/en.json";
import fr from "./locales/fr.json";

/** Langues proposées : [code, nom dans sa propre langue]. */
export const LOCALES = [
  ["fr", "Français"],
  ["en", "English"],
];
const CODES = LOCALES.map(([code]) => code);
const STORAGE_KEY = "nwasabi.lang";

/** Le code de langue pris en charge qui correspond à `tag` (`fr-CA` → `fr`), ou `undefined`. */
export function matchLocale(tag: string | null | undefined): string | undefined {
  const code = String(tag ?? "").toLowerCase().split(/[-_]/)[0];
  return CODES.includes(code) ? code : undefined;
}

function remembered() {
  try {
    return matchLocale(localStorage.getItem(STORAGE_KEY));
  } catch {
    return undefined; // stockage indisponible : on s'en passe
  }
}

export function detectLocale(search = location.search, languages = navigator.languages ?? []) {
  const asked = matchLocale(new URLSearchParams(search).get("lang"));
  if (asked) return asked;
  return remembered() ?? languages.map(matchLocale).find(Boolean) ?? "en";
}

export const i18n = createI18n({
  legacy: false,
  locale: detectLocale(),
  fallbackLocale: "en",
  messages: { fr, en },
});

document.documentElement.lang = i18n.global.locale.value;

/** Change de langue (et s'en souvient). */
export function setLocale(code: string): void {
  if (!CODES.includes(code)) return;
  i18n.global.locale.value = code as "fr" | "en";
  document.documentElement.lang = code;
  try {
    localStorage.setItem(STORAGE_KEY, code);
  } catch {
    /* rien à faire */
  }
}
