// Reconnaissance des gestes tactiles : appui, double appui, appui long et balayage, à un, deux ou trois
// doigts. Ne dépend ni du navigateur ni de la calculatrice : elle reçoit des positions de doigts (avec
// l'heure) et rend des gestes. Le lien avec les touches est fait par `InputMapper`.

import type { GestureName, TouchRule, TouchSettings } from "./keymap.js";

/** Minuteries et heure, remplaçables pour les tests. */
export interface Clock {
  /** Heure en millisecondes (croissante). */
  now(): number;
  setTimeout(callback: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
}

export const systemClock: Clock = {
  now: () => (typeof performance !== "undefined" ? performance.now() : Date.now()),
  setTimeout: (callback, ms) => setTimeout(callback, ms),
  clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

/**
 * - `instant` : le geste est fini (appui, balayage, appui long sans « tant que le doigt reste posé ») ;
 * - `start` / `end` : un appui long dont la touche reste enfoncée jusqu'à ce que le doigt se lève.
 */
export type GesturePhase = "instant" | "start" | "end";

export type GestureHandler = (gesture: GestureName, fingers: number, phase: GesturePhase) => void;

interface Finger {
  x0: number;
  y0: number;
  x: number;
  y: number;
  /** Le trajet du doigt, un point tous les `STEP` pixels au moins (pour reconnaître un demi-tour). */
  path: Array<[number, number]>;
}

/** Distance minimale entre deux points du trajet : en dessous, le tremblement du doigt fausserait la direction. */
const STEP = 10;
/** Un demi-tour tourne d'au moins ce nombre de degrés (180 pour un demi-cercle parfait, un peu moins à la main) … */
const UTURN_MIN_DEGREES = 150;
/** … et d'au plus celui-ci : au-delà, c'est un cercle, pas un demi-tour. */
const UTURN_MAX_DEGREES = 280;
/** Le trajet doit être au moins aussi long que cette fois la distance d'un balayage. */
const UTURN_MIN_LENGTH = 1.5;
/** Un trajet en zigzag (qui tourne tantôt d'un côté, tantôt de l'autre) n'est pas un demi-tour. */
const UTURN_STRAIGHTNESS = 1.6;

/** Le plus petit angle entre deux directions, signé, en radians (de −π à π). */
const turnBetween = (a: number, b: number): number => {
  let d = b - a;
  while (d > Math.PI) d -= 2 * Math.PI;
  while (d <= -Math.PI) d += 2 * Math.PI;
  return d;
};

/**
 * Le trajet est-il un demi-tour : une courbe qui tourne de la moitié d'un cercle (de 150° à 280°), toujours du même côté ?
 * Un quart de cercle (90°), une ligne droite ou un zigzag n'en sont pas.
 */
export function isUTurn(path: ReadonlyArray<readonly [number, number]>, swipeDistance: number): boolean {
  if (path.length < 3) return false;
  let length = 0;
  const headings: number[] = [];
  for (let i = 1; i < path.length; i++) {
    const [x0, y0] = path[i - 1]!;
    const [x1, y1] = path[i]!;
    length += Math.hypot(x1 - x0, y1 - y0);
    headings.push(Math.atan2(y1 - y0, x1 - x0));
  }
  if (length < swipeDistance * UTURN_MIN_LENGTH) return false;
  let signed = 0;
  let total = 0;
  for (let i = 1; i < headings.length; i++) {
    const turn = turnBetween(headings[i - 1]!, headings[i]!);
    signed += turn;
    total += Math.abs(turn);
  }
  const degrees = (Math.abs(signed) * 180) / Math.PI;
  return degrees >= UTURN_MIN_DEGREES && degrees <= UTURN_MAX_DEGREES && total <= Math.abs(signed) * UTURN_STRAIGHTNESS;
}

export class GestureRecognizer {
  private readonly fingers = new Map<number, Finger>();
  // Les doigts déjà levés pendant le geste (pour la distance parcourue).
  private readonly lifted = new Map<number, Finger>();
  private maxFingers = 0;
  private longHandle: unknown = null;
  /** Un appui long reconnu : `fingers` doigts, et s'il faut signaler sa fin. */
  private long: { fingers: number; held: boolean } | null = null;
  private lastTap: { fingers: number; time: number } | null = null;
  private tapHandle: unknown = null;

  constructor(
    private rules: readonly TouchRule[],
    private settings: TouchSettings,
    private readonly handler: GestureHandler,
    private readonly clock: Clock = systemClock,
  ) {}

  /** Change les règles (les gestes en cours sont abandonnés). */
  configure(rules: readonly TouchRule[], settings: TouchSettings): void {
    this.cancel();
    this.rules = rules;
    this.settings = settings;
  }

  private rule(gesture: GestureName, fingers: number): TouchRule | undefined {
    return this.rules.find((r) => r.gesture === gesture && r.fingers === fingers);
  }

  private wants(gesture: GestureName): boolean {
    return this.rules.some((r) => r.gesture === gesture);
  }

  down(id: number, x: number, y: number): void {
    if (this.fingers.size === 0) this.begin();
    this.fingers.set(id, { x0: x, y0: y, x, y, path: [[x, y]] });
    this.maxFingers = Math.max(this.maxFingers, this.fingers.size);
  }

  move(id: number, x: number, y: number): void {
    const finger = this.fingers.get(id);
    if (!finger) return;
    finger.x = x;
    finger.y = y;
    this.extend(finger, x, y);
  }

  /** Ajoute un point au trajet si le doigt a assez avancé depuis le dernier. */
  private extend(finger: Finger, x: number, y: number): void {
    const [lastX, lastY] = finger.path[finger.path.length - 1]!;
    if (Math.hypot(x - lastX, y - lastY) >= STEP) finger.path.push([x, y]);
  }

  up(id: number, x?: number, y?: number): void {
    const finger = this.fingers.get(id);
    if (!finger) return;
    if (x !== undefined && y !== undefined) {
      finger.x = x;
      finger.y = y;
      this.extend(finger, x, y);
    }
    // Le geste se juge quand le dernier doigt se lève, avec les positions de tous les doigts.
    if (this.fingers.size > 1) {
      this.lifted.set(id, finger);
      this.fingers.delete(id);
      return;
    }
    this.lifted.set(id, finger);
    this.fingers.delete(id);
    this.finish();
  }

  /** Abandonne le geste en cours, sans rien signaler (sauf la fin d'un appui long tenu). */
  cancel(): void {
    this.clearLong();
    if (this.long?.held) this.handler("long_press", this.long.fingers, "end");
    this.long = null;
    this.fingers.clear();
    this.lifted.clear();
    this.maxFingers = 0;
    if (this.tapHandle !== null) this.clock.clearTimeout(this.tapHandle);
    this.tapHandle = null;
    this.lastTap = null;
  }

  private begin(): void {
    this.lifted.clear();
    this.maxFingers = 0;
    this.long = null;
    this.clearLong();
    if (this.wants("long_press")) {
      this.longHandle = this.clock.setTimeout(() => this.onLongPress(), this.settings.longPressMs);
    }
  }

  private clearLong(): void {
    if (this.longHandle !== null) this.clock.clearTimeout(this.longHandle);
    this.longHandle = null;
  }

  /** Déplacement moyen de tous les doigts du geste depuis leur pose. */
  private displacement(): { dx: number; dy: number } {
    const all = [...this.lifted.values(), ...this.fingers.values()];
    if (all.length === 0) return { dx: 0, dy: 0 };
    const dx = all.reduce((sum, f) => sum + (f.x - f.x0), 0) / all.length;
    const dy = all.reduce((sum, f) => sum + (f.y - f.y0), 0) / all.length;
    return { dx, dy };
  }

  private moved(): boolean {
    const { dx, dy } = this.displacement();
    return Math.hypot(dx, dy) >= this.settings.swipeDistance;
  }

  /** Le trajet du doigt qui a le plus bougé est-il un demi-tour ? */
  private uTurn(): boolean {
    const all = [...this.lifted.values(), ...this.fingers.values()];
    return all.some((f) => isUTurn(f.path, this.settings.swipeDistance));
  }

  private onLongPress(): void {
    this.longHandle = null;
    // Un doigt qui a déjà bougé comme pour un balayage n'est plus un appui.
    if (this.moved()) return;
    const rule = this.rule("long_press", Math.min(this.maxFingers, 3));
    if (!rule) return;
    this.long = { fingers: rule.fingers, held: rule.whileHeld };
    this.handler("long_press", rule.fingers, rule.whileHeld ? "start" : "instant");
  }

  private finish(): void {
    this.clearLong();
    const fingers = Math.min(this.maxFingers, 3);
    if (this.long) {
      if (this.long.held) this.handler("long_press", this.long.fingers, "end");
      this.long = null;
      return;
    }
    if (this.rule("u_turn", fingers) && this.uTurn()) {
      this.handler("u_turn", fingers, "instant");
      return;
    }
    const { dx, dy } = this.displacement();
    if (Math.hypot(dx, dy) >= this.settings.swipeDistance) {
      const gesture: GestureName =
        Math.abs(dx) >= Math.abs(dy) ? (dx > 0 ? "swipe_right" : "swipe_left") : dy > 0 ? "swipe_down" : "swipe_up";
      if (this.rule(gesture, fingers)) this.handler(gesture, fingers, "instant");
      return;
    }
    this.tap(fingers);
  }

  private tap(fingers: number): void {
    const now = this.clock.now();
    const twice = this.rule("double_tap", fingers);
    if (twice && this.lastTap?.fingers === fingers && now - this.lastTap.time <= this.settings.doubleTapMs) {
      if (this.tapHandle !== null) this.clock.clearTimeout(this.tapHandle);
      this.tapHandle = null;
      this.lastTap = null;
      this.handler("double_tap", fingers, "instant");
      return;
    }
    const once = this.rule("tap", fingers);
    if (!twice) {
      if (once) this.handler("tap", fingers, "instant");
      return;
    }
    // Un double appui est possible : l'appui simple attend de savoir si un second suit.
    this.lastTap = { fingers, time: now };
    if (once) {
      this.tapHandle = this.clock.setTimeout(() => {
        this.tapHandle = null;
        this.lastTap = null;
        this.handler("tap", fingers, "instant");
      }, this.settings.doubleTapMs);
    }
  }
}
