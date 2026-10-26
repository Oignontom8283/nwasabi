// Ce que la page charge toute seule au démarrage : `?nwa=`, `?data=`, `?nmk=`, `?nfs=`, `?nds=`.
// Les adresses sont relatives à la page ou absolues ; un autre site doit autoriser la lecture (CORS).

export const FILE_PARAMS = ["nwa", "data", "nmk", "nfs", "nds"];

/** Les adresses données dans l'URL de la page (`null` si absentes). */
export function launchParams(search = location.search) {
  const params = new URLSearchParams(search);
  return Object.fromEntries(FILE_PARAMS.map((name) => [name, params.get(name)]));
}

/** Erreur de téléchargement : `url` dit laquelle des adresses a échoué. */
export class FetchError extends Error {
  constructor(url, reason) {
    super(`${url} : ${reason}`);
    this.url = url;
    this.reason = reason;
  }
}

/** Télécharge un fichier. */
export async function fetchFile(url) {
  let reply;
  try {
    reply = await fetch(url, { mode: "cors" });
  } catch {
    throw new FetchError(url, "network");
  }
  if (!reply.ok) throw new FetchError(url, `http:${reply.status}`);
  return reply;
}

/** Le nom de fichier d'une adresse (`https://x.org/a/jeu.nwa?v=2` → `jeu.nwa`). */
export function fileNameOf(url) {
  const path = new URL(url, location.href).pathname;
  return decodeURIComponent(path.slice(path.lastIndexOf("/") + 1)) || "application.nwa";
}
