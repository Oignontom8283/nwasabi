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
    this.fingers.set(id, { x0: x, y0: y, x, y });
    this.maxFingers = Math.max(this.maxFingers, this.fingers.size);
  }

  move(id: number, x: number, y: number): void {
    const finger = this.fingers.get(id);
    if (!finger) return;
    finger.x = x;
    finger.y = y;
  }

  up(id: number, x?: number, y?: number): void {
    const finger = this.fingers.get(id);
    if (!finger) return;
    if (x !== undefined && y !== undefined) {
      finger.x = x;
      finger.y = y;
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
