// Petit stockage clé-valeur dans IndexedDB : pour ce qui est trop gros pour `localStorage` (états enregistrés, applications
// récentes). Toutes les fonctions rendent `null` ou `false` au lieu de lever une erreur quand le stockage est indisponible
// (navigation privée, stockage bloqué) : l'interface s'en passe.

const DB = "nwasabi";
const STORE = "kv";

function open(): Promise<IDBDatabase | null> {
  return new Promise((resolve) => {
    try {
      const request = indexedDB.open(DB, 1);
      request.onupgradeneeded = () => request.result.createObjectStore(STORE);
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => resolve(null);
      request.onblocked = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
}

async function run<T>(mode: IDBTransactionMode, task: (store: IDBObjectStore) => IDBRequest<T>): Promise<T | null> {
  const db = await open();
  if (!db) return null;
  return new Promise((resolve) => {
    try {
      const request = task(db.transaction(STORE, mode).objectStore(STORE));
      // La connexion se ferme après chaque opération : elles ne s'accumulent pas et ne bloquent pas une future version de la base.
      request.onsuccess = () => (db.close(), resolve(request.result));
      request.onerror = () => (db.close(), resolve(null));
    } catch {
      db.close();
      resolve(null);
    }
  });
}

export const idbGet = <T>(key: string): Promise<T | null> => run<T>("readonly", (s) => s.get(key) as IDBRequest<T>).then((v) => v ?? null);
export const idbSet = async (key: string, value: unknown): Promise<boolean> => (await run("readwrite", (s) => s.put(value, key))) !== null;
export const idbDelete = async (key: string): Promise<boolean> => (await run("readwrite", (s) => s.delete(key))) !== null;
