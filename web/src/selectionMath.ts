// Les calculs d'une sélection de lignes (clic, Maj + clic, rectangle), sans rien d'autre : testables seuls.

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Les deux boîtes se recouvrent-elles (en largeur et en hauteur) ? */
export const intersects = (a: Box, b: Box): boolean => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** Le rectangle recouvre-t-il au moins une des boîtes ? */
export const intersectsAny = (rect: Box, boxes: Box[]): boolean => boxes.some((b) => intersects(rect, b));

/** La plus petite boîte qui contient toutes les boîtes données (`null` s'il n'y en a pas). */
export function union(boxes: Box[]): Box | null {
  if (!boxes.length) return null;
  const left = Math.min(...boxes.map((b) => b.x));
  const top = Math.min(...boxes.map((b) => b.y));
  const right = Math.max(...boxes.map((b) => b.x + b.w));
  const bottom = Math.max(...boxes.map((b) => b.y + b.h));
  return { x: left, y: top, w: right - left, h: bottom - top };
}

/** Le rectangle tiré de `from` à `to` (dans n'importe quel sens). */
export const rectBetween = (from: { x: number; y: number }, to: { x: number; y: number }): Box => ({
  x: Math.min(from.x, to.x),
  y: Math.min(from.y, to.y),
  w: Math.abs(to.x - from.x),
  h: Math.abs(to.y - from.y),
});

/** Les clés de `from` à `to` incluses, dans l'ordre de la liste (vide si l'une des deux n'y est pas). */
export function rangeKeys<K>(keys: K[], from: K, to: K): K[] {
  const a = keys.indexOf(from);
  const b = keys.indexOf(to);
  if (a < 0 || b < 0) return [];
  return keys.slice(Math.min(a, b), Math.max(a, b) + 1);
}

/** La clé voisine de `current` (`delta` lignes plus loin), bornée à la liste ; la première si `current` est absente. */
export function neighbour<K>(keys: K[], current: K | null, delta: number): K | null {
  if (!keys.length) return null;
  const at = current === null ? -1 : keys.indexOf(current);
  if (at < 0) return keys[delta < 0 ? keys.length - 1 : 0] ?? null;
  return keys[Math.max(0, Math.min(keys.length - 1, at + delta))] ?? null;
}
