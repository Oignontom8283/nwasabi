// Du clavier de l'ordinateur et des gestes tactiles aux touches de la calculatrice, selon un `KeyMap`.
//
// Plusieurs entrées peuvent viser la même touche de la calculatrice (deux touches de l'ordinateur, un geste) :
// elle reste enfoncée tant qu'au moins une d'elles est tenue.

import { GestureRecognizer, systemClock } from "./gestures.js";
import type { Clock, GesturePhase } from "./gestures.js";
import type { KeyName } from "./keys.js";
import { KeyMap } from "./keymap.js";
import type { GestureName } from "./keymap.js";

/** Ce que l'on pilote : le lecteur (`Player`). */
export interface KeySink {
  key(name: KeyName, down: boolean): void;
}

/** Les champs utiles d'un `KeyboardEvent`. */
export interface HostKeyEvent {
  code: string;
  key: string;
}

/** Les champs utiles d'un `PointerEvent`. */
export interface HostPointerEvent {
  pointerId: number;
  clientX: number;
  clientY: number;
  /** "touch", "pen" ou "mouse" : un clic de souris n'est ni un appui, ni un appui long (on clique sur l'écran pour y mettre le focus). */
  pointerType?: string;
}

/** Ce qu'il faut d'un élément ou d'une fenêtre pour y brancher les événements. */
export interface EventSource {
  addEventListener(type: string, listener: (event: any) => void): void;
  removeEventListener(type: string, listener: (event: any) => void): void;
}

export class InputMapper {
  private map: KeyMap;
  private readonly recognizer: GestureRecognizer;
  private readonly counts = new Map<KeyName, number>();
  /** Touches de l'ordinateur enfoncées → touches de la calculatrice qu'elles tiennent (mapping d'alors). */
  private readonly held = new Map<string, readonly KeyName[]>();
  private readonly gestureHeld = new Map<GestureName, readonly KeyName[]>();
  /** Le type de chaque pointeur posé, et si le geste en cours a commencé à la souris. */
  private readonly pointerKinds = new Map<number, string>();
  private mouseGesture = false;

  constructor(
    private readonly sink: KeySink,
    map: KeyMap = KeyMap.default(),
    private readonly clock: Clock = systemClock,
  ) {
    this.map = map;
    this.recognizer = new GestureRecognizer(
      map.rules,
      map.settings,
      (gesture, fingers, phase) => this.onGesture(gesture, fingers, phase),
      clock,
    );
  }

  get keymap(): KeyMap {
    return this.map;
  }

  /** Change de mapping ; ce qui était tenu est relâché. */
  setKeyMap(map: KeyMap): void {
    this.releaseAll();
    this.map = map;
    this.recognizer.configure(map.rules, map.settings);
  }

  /** Relâche tout ce que les entrées tiennent. */
  releaseAll(): void {
    this.recognizer.cancel();
    for (const keys of [...this.held.values(), ...this.gestureHeld.values()]) this.releaseKeys(keys);
    this.held.clear();
    this.gestureHeld.clear();
  }

  private pressKeys(keys: readonly KeyName[]): void {
    for (const key of keys) {
      const count = this.counts.get(key) ?? 0;
      this.counts.set(key, count + 1);
      if (count === 0) this.sink.key(key, true);
    }
  }

  private releaseKeys(keys: readonly KeyName[]): void {
    for (const key of keys) {
      const count = (this.counts.get(key) ?? 0) - 1;
      if (count > 0) {
        this.counts.set(key, count);
      } else {
        this.counts.delete(key);
        this.sink.key(key, false);
      }
    }
  }

  /**
   * Une touche de l'ordinateur est enfoncée (`down`) ou relâchée. Rend `true` si le mapping la connaît
   * (l'appelant peut alors empêcher le comportement habituel du navigateur).
   */
  hostKey(event: HostKeyEvent, down: boolean): boolean {
    const id = event.code || `key:${event.key}`;
    if (down) {
      if (this.held.has(id)) return true; // répétition automatique de la touche
      const keys = this.map.resolve(event.code, event.key);
      if (!keys) return false;
      this.held.set(id, keys);
      this.pressKeys(keys);
      return true;
    }
    const keys = this.held.get(id);
    if (!keys) return false;
    this.held.delete(id);
    this.releaseKeys(keys);
    return true;
  }

  /** Événements de pointeur (doigt, stylet, souris) sur l'écran, pour les gestes. */
  pointerDown(event: HostPointerEvent): void {
    if (this.pointerKinds.size === 0) this.mouseGesture = event.pointerType === "mouse";
    this.pointerKinds.set(event.pointerId, event.pointerType ?? "");
    this.recognizer.down(event.pointerId, event.clientX, event.clientY);
  }

  pointerMove(event: HostPointerEvent): void {
    this.recognizer.move(event.pointerId, event.clientX, event.clientY);
  }

  pointerUp(event: HostPointerEvent): void {
    this.pointerKinds.delete(event.pointerId);
    this.recognizer.up(event.pointerId, event.clientX, event.clientY);
  }

  pointerCancel(): void {
    this.pointerKinds.clear();
    this.recognizer.cancel();
  }

  private onGesture(gesture: GestureName, fingers: number, phase: GesturePhase): void {
    // À la souris, seuls les gestes tracés (balayage, demi-tour) comptent : un simple clic ne doit rien appuyer.
    if (this.mouseGesture && (gesture === "tap" || gesture === "double_tap" || gesture === "long_press")) return;
    const rule = this.map.rules.find((r) => r.gesture === gesture && r.fingers === fingers);
    if (!rule) return;
    if (phase === "end") {
      const keys = this.gestureHeld.get(gesture);
      if (keys) this.releaseKeys(keys);
      this.gestureHeld.delete(gesture);
      return;
    }
    this.pressKeys(rule.keys);
    if (phase === "start") {
      this.gestureHeld.set(gesture, rule.keys);
    } else {
      this.clock.setTimeout(() => this.releaseKeys(rule.keys), this.map.settings.pressMs);
    }
  }

  /**
   * Branche le clavier de l'ordinateur sur `target` (souvent `window`). `ignore` écarte des événements
   * (par exemple ceux d'un champ de saisie). Rend la fonction qui débranche.
   */
  attachKeyboard(target: EventSource, ignore?: (event: KeyboardEvent) => boolean): () => void {
    const handler = (down: boolean) => (event: KeyboardEvent) => {
      if (ignore?.(event)) return;
      if (this.hostKey(event, down)) event.preventDefault();
    };
    const keydown = handler(true);
    const keyup = handler(false);
    target.addEventListener("keydown", keydown);
    target.addEventListener("keyup", keyup);
    return () => {
      target.removeEventListener("keydown", keydown);
      target.removeEventListener("keyup", keyup);
    };
  }

  /**
   * Branche les gestes tactiles sur `target` (souvent l'écran). Pour que le navigateur ne fasse pas défiler
   * la page pendant un balayage, l'élément doit avoir `touch-action: none`. Rend la fonction qui débranche.
   */
  attachPointer(target: EventSource): () => void {
    const down = (e: PointerEvent) => {
      (e.currentTarget as Element | null)?.setPointerCapture?.(e.pointerId);
      this.pointerDown(e);
    };
    const move = (e: PointerEvent) => this.pointerMove(e);
    const up = (e: PointerEvent) => this.pointerUp(e);
    const cancel = () => this.pointerCancel();
    target.addEventListener("pointerdown", down);
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
    target.addEventListener("pointercancel", cancel);
    return () => {
      target.removeEventListener("pointerdown", down);
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
      target.removeEventListener("pointercancel", cancel);
    };
  }
}
