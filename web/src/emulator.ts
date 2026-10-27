// Lien entre l'émulateur et l'interface Vue. L'émulation tourne dans un Worker (`emulator.worker.ts`) pour ne jamais
// figer la page : ici on ne garde que l'état réactif (mis à jour par ce que le Worker envoie), le mapping des touches et
// les entrées (clavier, souris, doigts), qui dépendent de la page. Parler au module est donc asynchrone.

import { inject, reactive } from "vue";
import { InputMapper, KeyMap, Player } from "@nwasabi/js";
import type { Firmware, KeyName, Model, PlayerState, Reply, Request, SpeedMode } from "@nwasabi/js";
// Le module WebAssembly construit localement (voir vite.config.ts) : on n'utilise pas celui de @nwasabi/wasm.
import wasmUrl from "nwasabi.wasm?url";
import type { FromWorker, PlayerMethod, ToWorker } from "./worker-protocol.js";

export function useEmulator() {
  // `keymapVersion` change à chaque nouveau mapping des touches : les vues qui l'affichent le suivent.
  const state = reactive({ ...Player.initialState(), ready: false, keymapVersion: 0, mips: 0, slow: false });

  let worker: Worker | null = null;
  let nextId = 1;
  const pending = new Map<number, { resolve: (result: unknown) => void; reject: (error: Error) => void }>();
  let canvas: HTMLCanvasElement | null = null;
  let gestureTarget: HTMLElement | null = null;
  let detachPointer: (() => void) | null = null;
  let transferred = false;

  // Les entrées : le mapping reste dans la page ; ce qui en sort part au Worker comme une touche de la calculatrice.
  const input = new InputMapper({ key: (name, down) => void player("key", name, down) }, KeyMap.default());
  state.keymapName = input.keymap.name;

  function send(message: ToWorker, transfer: Transferable[] = []): void {
    worker?.postMessage(message, transfer);
  }

  function ask<T>(build: (id: number) => ToWorker, transfer: Transferable[] = []): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      if (!worker) return reject(new Error("émulateur pas encore prêt"));
      const id = nextId++;
      pending.set(id, { resolve: resolve as (result: unknown) => void, reject });
      send(build(id), transfer);
    });
  }

  function onMessage(event: MessageEvent<FromWorker>): void {
    const message = event.data;
    if (message.kind === "state") {
      Object.assign(state, message.patch);
      return;
    }
    const waiting = pending.get(message.id);
    if (!waiting) return;
    pending.delete(message.id);
    if (message.kind === "reply") waiting.resolve(message.result);
    else waiting.reject(new Error(message.message));
  }

  /** Appelle une méthode du lecteur, dans le Worker. Les erreurs vont dans `state.error`. */
  function player<T = unknown>(method: PlayerMethod, ...args: unknown[]): Promise<T | undefined> {
    return ask<T>((id) => ({ kind: "player", id, method, args })).catch((error: Error) => {
      state.error = error.message;
      return undefined;
    });
  }

  /** Donne l'écran au Worker (une seule fois : un canvas ne se transfère qu'une fois). */
  function transferCanvas(): OffscreenCanvas | null {
    if (transferred || !canvas || !("transferControlToOffscreen" in canvas)) return null;
    transferred = true;
    return canvas.transferControlToOffscreen();
  }

  async function init(): Promise<void> {
    if (typeof Worker === "undefined") throw new Error("ce navigateur ne sait pas lancer de Worker");
    worker = new Worker(new URL("./emulator.worker.ts", import.meta.url), { type: "module" });
    worker.onmessage = onMessage;
    worker.onerror = (event) => {
      state.error = event.message || "erreur de l'émulateur";
    };
    const offscreen = transferCanvas();
    const wasm = new URL(wasmUrl, location.href).href;
    await ask<boolean>((id) => ({ kind: "init", id, wasmUrl: wasm, canvas: offscreen }), offscreen ? [offscreen] : []);
    state.ready = true;
  }

  /** Les gestes tactiles se font sur l'écran : on y branche le mapping dès qu'on a la cible. */
  function attach(element: HTMLCanvasElement, gestures: HTMLElement = element): void {
    canvas = element;
    gestureTarget = gestures;
    if (!detachPointer) detachPointer = input.attachPointer(gestures);
  }

  /** Envoie une requête au module ; les erreurs sont gardées dans `state.error`. */
  const call = (request: Request): Promise<Reply> =>
    ask<Reply>((id) => ({ kind: "request", id, request, quiet: false })).catch((error: Error) => ({ ok: false, error: error.message }));

  /** Comme `call`, mais une erreur n'est pas affichée : pour les lectures périodiques des vues. */
  const quiet = (request: Request): Promise<Reply> =>
    ask<Reply>((id) => ({ kind: "request", id, request, quiet: true })).catch((error: Error) => ({ ok: false, error: error.message }));

  /** Le mapping des touches en vigueur. */
  const keymap = (): KeyMap => input.keymap;

  function setKeyMap(map: KeyMap): void {
    input.setKeyMap(map);
    state.keymapName = map.name;
    state.keymapVersion++;
  }

  /** Branche le clavier de l'ordinateur (voir `InputMapper.attachKeyboard`) ; rend la fonction qui débranche. */
  const attachKeyboard = (target: Window, ignore?: (event: KeyboardEvent) => boolean): (() => void) => input.attachKeyboard(target, ignore);

  /** Une touche de la calculatrice enfoncée ou relâchée par un clic sur l'écran, hors du mapping. */
  const press = (name: KeyName, down: boolean): void => void player("key", name, down);

  /** Les fichiers à remettre dans chaque appareil neuf (ceux conservés dans le navigateur), ou `null`. */
  const setKept = (document: unknown): void => send({ kind: "kept", document });

  /** L'écran en image PNG (le Worker le dessine : la page n'a pas ses pixels). */
  const capture = (): Promise<Blob | null> => ask<Blob | null>((id) => ({ kind: "capture", id })).catch(() => null);

  /** Commence l'enregistrement de l'écran en GIF ; `false` si c'est impossible. */
  const startRecording = (): Promise<boolean> => ask<boolean>((id) => ({ kind: "record", id, action: "start" })).catch(() => false);
  /** Termine l'enregistrement et rend le fichier GIF (`null` s'il est vide). */
  const stopRecording = (): Promise<Uint8Array | null> => ask<Uint8Array | null>((id) => ({ kind: "record", id, action: "stop" })).catch(() => null);

  return {
    state,
    init,
    call,
    quiet,
    attach,
    keymap,
    setKeyMap,
    attachKeyboard,
    press,
    setKept,
    capture,
    startRecording,
    stopRecording,
    load: (nwa: Uint8Array, name: string, data: Uint8Array | null = null) => player<boolean>("load", nwa, name, data),
    restart: () => player<boolean>("restart"),
    stepFrame: () => player<boolean>("stepFrame"),
    runWithoutData: () => player<boolean>("runWithoutData"),
    changeModel: (model: Model, firmware?: Firmware) => player("changeModel", model, firmware),
    setSpeed: (speed: SpeedMode) => player("setSpeed", speed),
    setCustomMhz: (mhz: number) => player("setCustomMhz", mhz),
    start: () => player("start"),
    stop: () => player("stop"),
    bump: () => player("bump"),
    dump: (parts: readonly string[]) => player<unknown>("dump", parts),
    restore: (document: unknown) => player<boolean>("restore", document),
    /** Remplace l'état d'un champ qui n'est que de la page (le Worker n'y touche pas). */
    patch: (fields: Partial<PlayerState>) => Object.assign(state, fields),
  };
}

/** L'émulateur tel que les vues le reçoivent. */
export type Emulator = ReturnType<typeof useEmulator>;

/** L'émulateur fourni par `App.vue`. */
export function useEmu(): Emulator {
  const emulator = inject<Emulator>("emulator");
  if (!emulator) throw new Error("émulateur non fourni");
  return emulator;
}
