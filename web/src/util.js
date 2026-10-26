// Petites fonctions communes aux vues.

export const hex = (n, width = 8) => (n >>> 0).toString(16).padStart(width, "0");
export const hex0x = (n, width = 8) => `0x${hex(n, width)}`;

/** Taille lisible : 1536 → "1,5 Kio". */
export function formatBytes(n, locale) {
  const units = ["o", "Kio", "Mio", "Gio"];
  let value = n;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const text = new Intl.NumberFormat(locale, { maximumFractionDigits: unit ? 1 : 0 }).format(value);
  return `${text} ${units[unit]}`;
}

/** Durée lisible à partir de millisecondes : 3723000 → "1:02:03". */
export function formatDuration(ms) {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const two = (n) => String(n).padStart(2, "0");
  return h ? `${h}:${two(m)}:${two(s)}` : `${m}:${two(s)}`;
}

/** Lit une adresse saisie : `0x20000000`, `20000000h`, ou décimal. `undefined` si invalide. */
export function parseAddress(text) {
  const t = String(text).trim().replace(/_/g, "");
  if (!t) return undefined;
  let n;
  if (/^0x[0-9a-f]+$/i.test(t)) n = parseInt(t.slice(2), 16);
  else if (/^[0-9a-f]+h$/i.test(t)) n = parseInt(t.slice(0, -1), 16);
  else if (/^\d+$/.test(t)) n = parseInt(t, 10);
  else return undefined;
  return Number.isFinite(n) && n >= 0 && n <= 0xffffffff ? n >>> 0 : undefined;
}

/** Télécharge un contenu sous ce nom. */
export function download(name, content, type = "application/octet-stream") {
  const blob = content instanceof Blob ? content : new Blob([content], { type });
  const url = URL.createObjectURL(blob);
  const link = Object.assign(document.createElement("a"), { href: url, download: name });
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Lit un fichier choisi par l'utilisateur. */
export const readBytes = async (file) => new Uint8Array(await file.arrayBuffer());

/** Le même nom sans son extension. */
export const stem = (name) => String(name).replace(/\.[^.]+$/, "");

/** Stockage du navigateur qui ne casse jamais (navigation privée, stockage bloqué…). */
export const remember = {
  get(key, fallback) {
    try {
      const value = localStorage.getItem(key);
      return value === null ? fallback : JSON.parse(value);
    } catch {
      return fallback;
    }
  },
  set(key, value) {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      /* rien à faire */
    }
  },
};
