// Lien avec le serveur `nwasabi` (`--nwa`, `--nmk`, `--nfs`, `--nds`) : il relit ses fichiers à chaque requête et donne
// leur empreinte sur `__nwasabi/version`. La page les compare une fois par seconde ; quand l'un a changé, elle le recharge
// tout de suite (si le réglage « recharger automatiquement » est activé) ou propose de le faire (bouton, ou Ctrl + R).
// Une page hébergée ailleurs n'a pas ce lien : rien n'est alors surveillé.

import { reactive } from "vue";

/** Ce que recharge chaque fichier : l'application (avec ses données), le mapping, les fichiers, l'état. */
export type Part = "app" | "nmk" | "nfs" | "nds";
type Stamps = Record<keyof typeof PARTS, string | null>;

const PARTS = { nwa: "app", data: "app", nmk: "nmk", nfs: "nfs", nds: "nds" } as const;

export const server = reactive<{ active: boolean; stale: Part[] }>({
  /** Le serveur `nwasabi` répond-il ? */
  active: false,
  /** Ce qui a changé sur le disque sans être rechargé. */
  stale: [],
});

const POLL_MS = 1000;
let known: Stamps | null = null; // empreintes de ce qui est chargé dans la page
let latest: Stamps | null = null; // empreintes vues au dernier relevé
let timer: ReturnType<typeof setInterval> | undefined;
let onChange: (part: Part) => Promise<void> | void = () => {};
let auto: () => boolean = () => false;

/** Une réponse JSON de `nwasabi`, ou `null` (autre serveur, hors ligne). */
async function json(path: string): Promise<Response | null> {
  try {
    const reply = await fetch(path, { cache: "no-store" });
    return reply.headers.get("content-type")?.includes("application/json") ? reply : null;
  } catch {
    return null;
  }
}

async function stamps(): Promise<Stamps | null> {
  const reply = await json("__nwasabi/version");
  return reply?.ok ? reply.json() : null;
}

/** Relevé de ce que la page va charger ; à faire avant le premier chargement. */
export async function baseline() {
  known = latest = await stamps();
  server.active = known !== null;
}

/**
 * Lance la surveillance. `onChange(part)` recharge une partie ; `autoReload()` dit si le réglage est activé.
 */
export function watch(change: (part: Part) => Promise<void> | void, autoReload: () => boolean): void {
  onChange = change;
  auto = autoReload;
  clearInterval(timer);
  if (!server.active) return;
  timer = setInterval(poll, POLL_MS);
}

async function poll() {
  if (document.hidden) return;
  const now = await stamps();
  if (!now) return;
  latest = now;
  const keys = Object.keys(PARTS) as Array<keyof typeof PARTS>;
  const changed = [...new Set(keys.filter((key) => now[key] !== known?.[key]).map((key) => PARTS[key]))];
  server.stale = changed;
  if (changed.length && auto()) await reload();
}

/** Recharge tout ce qui a changé. */
export async function reload() {
  const parts = [...server.stale];
  if (!parts.length) return;
  known = latest;
  server.stale = [];
  for (const part of parts) await onChange(part);
}

export function stop() {
  clearInterval(timer);
}

/** Lit une réponse du serveur : le texte, ou l'erreur (`{ error }` du code 422), ou `null` si le serveur ne l'a pas. */
export async function fetchPart(path: string): Promise<{ reply: Response } | { error: string } | null> {
  const reply = await json(path);
  if (reply?.ok) return { reply };
  if (reply && reply.status === 422) return { error: (await reply.json()).error };
  return null;
}
