// Ce que la page charge toute seule au démarrage : les paramètres d'adresse `?nwa=` et `?nmk=`.
// Les adresses sont relatives à la page ou absolues ; un autre site doit autoriser la lecture (CORS).

/** Les adresses données dans l'URL de la page (`null` si absentes). */
export function launchParams(search = location.search) {
  const params = new URLSearchParams(search);
  return { nwa: params.get("nwa"), nmk: params.get("nmk") };
}

/** Télécharge un fichier ; l'erreur dit laquelle des adresses a échoué. */
export async function fetchFile(url) {
  let reply;
  try {
    reply = await fetch(url, { mode: "cors" });
  } catch {
    throw new Error(`${url} : téléchargement impossible (réseau, ou le site n'autorise pas la lecture : CORS)`);
  }
  if (!reply.ok) throw new Error(`${url} : réponse HTTP ${reply.status}`);
  return reply;
}

/** Le nom de fichier d'une adresse (`https://x.org/a/jeu.nwa?v=2` → `jeu.nwa`). */
export function fileNameOf(url) {
  const path = new URL(url, location.href).pathname;
  return decodeURIComponent(path.slice(path.lastIndexOf("/") + 1)) || "application.nwa";
}
