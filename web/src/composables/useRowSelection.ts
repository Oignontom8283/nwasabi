// Sélection de lignes comme dans un explorateur de fichiers : clic, Ctrl + clic (ajoute ou retire), Maj + clic (plage), flèches (avec
// Maj pour étendre), Ctrl + A, clic dans le vide (désélectionne) et rectangle tiré dans l'espace libre. Le rectangle ne démarre que sur
// du vide (la barre libre d'une ligne ou le fond de la liste) : appuyer sur du texte garde la sélection de texte du navigateur, et
// pendant un rectangle le navigateur ne sélectionne ni ne glisse rien. Une ligne est prise quand le rectangle touche l'un de ses
// textes (ou l'une de ses icônes), pas quand il passe seulement dans sa largeur.

import { ref } from "vue";
import type { Ref } from "vue";
import { intersectsAny, neighbour, rangeKeys, rectBetween } from "../selectionMath.js";
import type { Box } from "../selectionMath.js";

export interface RowSelectionOptions<K extends string | number> {
  /** La zone qui contient les lignes (elle défile, et le rectangle se dessine dedans). */
  container: Ref<HTMLElement | null>;
  /** Les clés des lignes, dans l'ordre affiché. */
  keys: () => K[];
  /** Attribut `data-…` qui porte la clé sur chaque ligne (ex. `data-addr`). */
  attribute: string;
  /** Relit la clé depuis la valeur de l'attribut. */
  parse: (raw: string) => K;
}

/** Au-delà de cette distance (pixels), un appui sur du vide devient un rectangle ; en deçà, c'est un clic. */
const THRESHOLD = 4;

export function useRowSelection<K extends string | number>(options: RowSelectionOptions<K>) {
  const selected = ref(new Set<K>()) as Ref<Set<K>>;
  const band = ref<Box | null>(null);
  /** La ligne de départ des plages (Maj + clic) et la ligne « active » des flèches. */
  let anchor: K | null = null;
  const caret = ref<K | null>(null) as Ref<K | null>;
  let start: { x: number; y: number; id: number } | null = null;
  let base = new Set<K>();
  let moved = false;
  /** L'appui a eu lieu sur le fond de la liste (pas sur une ligne). */
  let onBackground = false;

  const set = (keys: Iterable<K>) => (selected.value = new Set(keys));
  const clear = () => set([]);
  const selectAll = () => set(options.keys());
  const only = (key: K) => {
    set([key]);
    anchor = caret.value = key;
  };

  /** Un clic sur une ligne, avec ses touches modificatrices. */
  function click(key: K, event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) {
    if (event.ctrlKey || event.metaKey) {
      const next = new Set(selected.value);
      if (!next.delete(key)) next.add(key);
      selected.value = next;
      anchor = key;
    } else if (event.shiftKey && anchor !== null) set(rangeKeys(options.keys(), anchor, key));
    else only(key);
    caret.value = key;
  }

  /** Flèche haut ou bas : déplace la ligne active ; avec `extend`, étend la sélection depuis le point de départ. Rend la nouvelle ligne. */
  function move(delta: number, extend: boolean): K | null {
    const next = neighbour(options.keys(), caret.value, delta);
    if (next === null) return null;
    if (extend && anchor !== null) set(rangeKeys(options.keys(), anchor, next));
    else only(next);
    caret.value = next;
    return next;
  }

  const fromRect = (r: DOMRect, area: HTMLElement): Box => {
    const origin = area.getBoundingClientRect();
    return { x: r.left - origin.left + area.scrollLeft, y: r.top - origin.top + area.scrollTop, w: r.width, h: r.height };
  };

  /**
   * Les boîtes du contenu d'une ligne : celles de ses textes, au plus près des lettres (pas la largeur de la colonne), et celles
   * de ses éléments sans texte (une icône). Les éléments marqués `data-empty` (une barre qui remplit la ligne) ne comptent pas.
   */
  function contentBoxes(row: HTMLElement, area: HTMLElement): Box[] {
    const boxes: Box[] = [];
    for (const child of row.children) {
      if (!(child instanceof HTMLElement) || child.hasAttribute("data-empty") || child.offsetWidth === 0) continue;
      if (child.textContent?.trim()) {
        const range = document.createRange();
        range.selectNodeContents(child);
        for (const r of range.getClientRects()) if (r.width > 0 && r.height > 0) boxes.push(fromRect(r, area));
      } else boxes.push(fromRect(child.getBoundingClientRect(), area));
    }
    return boxes.length ? boxes : [fromRect(row.getBoundingClientRect(), area)];
  }

  const emptySpace = (target: EventTarget | null, area: HTMLElement) =>
    target === area || (target instanceof HTMLElement && (target.hasAttribute(options.attribute) || target.hasAttribute("data-empty")) && area.contains(target));

  function pointerDown(event: PointerEvent) {
    const area = options.container.value;
    if (!area || event.button !== 0 || !emptySpace(event.target, area)) return;
    // Un rectangle de sélection n'est pas une sélection de texte : le navigateur ne doit pas commencer la sienne (ni la glisser).
    event.preventDefault();
    getSelection()?.removeAllRanges();
    area.focus({ preventScroll: true });
    area.style.userSelect = "none";
    // Le relâchement peut avoir lieu hors de la liste : on le suit sur la fenêtre pour toujours rendre la sélection de texte.
    window.addEventListener("pointerup", pointerUp, { once: true });
    window.addEventListener("pointercancel", pointerUp, { once: true });
    onBackground = event.target === area;
    const origin = area.getBoundingClientRect();
    start = { x: event.clientX - origin.left + area.scrollLeft, y: event.clientY - origin.top + area.scrollTop, id: event.pointerId };
    base = event.ctrlKey || event.metaKey || event.shiftKey ? new Set(selected.value) : new Set();
    moved = false;
  }

  function pointerMove(event: PointerEvent) {
    const area = options.container.value;
    if (!area || !start) return;
    const origin = area.getBoundingClientRect();
    const point = { x: event.clientX - origin.left + area.scrollLeft, y: event.clientY - origin.top + area.scrollTop };
    if (!moved) {
      if (Math.hypot(point.x - start.x, point.y - start.y) < THRESHOLD) return;
      moved = true;
      area.setPointerCapture(start.id);
      if (!base.size) clear();
    }
    const rect = rectBetween(start, point);
    band.value = rect;
    const next = new Set(base);
    for (const row of area.querySelectorAll<HTMLElement>(`[${options.attribute}]`)) {
      if (intersectsAny(rect, contentBoxes(row, area))) next.add(options.parse(row.getAttribute(options.attribute) ?? ""));
    }
    selected.value = next;
  }

  function pointerUp() {
    const area = options.container.value;
    if (area) area.style.userSelect = "";
    // Comme dans un explorateur : un clic dans le vide, sans rectangle ni touche, désélectionne tout.
    if (start && !moved && onBackground && !base.size) clear();
    start = null;
    band.value = null;
  }

  /** Le navigateur veut glisser le texte sélectionné : une ligne ne se dépose nulle part, on l'en empêche. */
  const dragStart = (event: Event) => event.preventDefault();

  /** Après un rectangle, le « clic » qui suit ne doit pas refaire une sélection simple : à tester dans le gestionnaire de clic. */
  const wasDrag = () => {
    const drag = moved;
    moved = false;
    return drag;
  };

  return { selected, band, caret, set, clear, selectAll, only, click, move, wasDrag, pointerDown, pointerMove, pointerUp, dragStart };
}
