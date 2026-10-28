// Ce que la page charge toute seule au démarrage : `?nwa=`, `?data=`, `?flash=`, `?nmk=`, `?nfs=`, `?nds=` ; `?model=` pour le modèle émulé et `?firmware=` pour la disposition du firmware.
// Les adresses sont relatives à la page ou absolues ; un autre site doit autoriser la lecture (CORS).

export const FILE_PARAMS = ["nwa", "data", "flash", "nmk", "nfs", "nds"] as const;
export type FileParam = (typeof FILE_PARAMS)[number];

/** Les adresses données dans l'URL de la page (`null` si absentes). */
export function launchParams(search = location.search): Record<FileParam, string | null> {
  const params = new URLSearchParams(search);
  return Object.fromEntries(FILE_PARAMS.map((name) => [name, params.get(name)])) as Record<FileParam, string | null>;
}

/** Le modèle demandé par `?model=` (`n0110`, `n0115` ou `n0120`), s'il est valide. */
export function launchModel(search = location.search): "n0110" | "n0115" | "n0120" | null {
  const value = new URLSearchParams(search).get("model")?.toLowerCase();
  return value === "n0110" || value === "n0115" || value === "n0120" ? value : null;
}

/** La disposition du firmware demandée par `?firmware=` (`24` ou `26.4.0`), si elle est valide. */
export function launchFirmware(search = location.search): "24" | "26.4.0" | null {
  const value = new URLSearchParams(search).get("firmware");
  return value === "24" || value === "26.4.0" ? value : null;
}

/** Erreur de téléchargement : `url` dit laquelle des adresses a échoué. */
export class FetchError extends Error {
  url: string;
  reason: string;
  constructor(url: string, reason: string) {
    super(`${url} : ${reason}`);
    this.url = url;
    this.reason = reason;
  }
}

/** Télécharge un fichier. */
export async function fetchFile(url: string): Promise<Response> {
  let reply: Response;
  try {
    reply = await fetch(url, { mode: "cors" });
  } catch {
    throw new FetchError(url, "network");
  }
  if (!reply.ok) throw new FetchError(url, `http:${reply.status}`);
  return reply;
}

/** Le nom de fichier d'une adresse (`https://x.org/a/jeu.nwa?v=2` → `jeu.nwa`). */
export function fileNameOf(url: string): string {
  const path = new URL(url, location.href).pathname;
  return decodeURIComponent(path.slice(path.lastIndexOf("/") + 1)) || "application.nwa";
}
