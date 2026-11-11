// Lien entre l'émulateur et l'interface Vue. L'émulation tourne dans un Worker (`emulator.worker.ts`) pour ne jamais
// figer la page : ici on ne garde que l'état réactif (mis à jour par ce que le Worker envoie), le mapping des touches et
// les entrées (clavier, souris, doigts), qui dépendent de la page. Parler au module est donc asynchrone.

import * as Comlink from "comlink";
import { inject, reactive } from "vue";
import { InputMapper, KeyMap, Player } from "@nwasabi/js";
import type { Firmware, KeyName, Model, PlayerState, Reply, Request, SpeedMode } from "@nwasabi/js";
// Le module WebAssembly construit localement (voir vite.config.ts) : on n'utilise pas celui de @nwasabi/wasm.
import wasmUrl from "nwasabi.wasm?url";
import type { PageApi, PlayerMethod, WorkerApi } from "./worker-protocol.js";

export function useEmulator() {
  // `keymapVersion` change à chaque nouveau mapping des touches : les vues qui l'affichent le suivent.
  const state = reactive({ ...Player.initialState(), ready: false, deviceSeq: 0, keymapVersion: 0, mips: 0, slow: false });

  let worker: Worker | null = null;
  /** Les fonctions du Worker, appelées comme les nôtres (`comlink` envoie les messages et rend les réponses). */
  let remote: Comlink.Remote<WorkerApi> | null = null;
  let canvas: HTMLCanvasElement | null = null;
  let detachPointer: (() => void) | null = null;

  // Les entrées : le mapping reste dans la page ; ce qui en sort part au Worker comme une touche de la calculatrice.
  const input = new InputMapper({ key: (name, down) => void player("key", name, down) }, KeyMap.default());
  state.keymapName = input.keymap.name;

  /** Le Worker, ou une erreur s'il n'est pas encore prêt. */
  function rpc(): Comlink.Remote<WorkerApi> {
    if (!remote) throw new Error("émulateur pas encore prêt");
    return remote;
  }

  /** Ce que le Worker appelle : les changements d'état et l'écran. */
  const pageApi: PageApi = {
    state(patch) {
      Object.assign(state, patch);
    },
    screen(bitmap) {
      canvas?.getContext("2d")?.drawImage(bitmap, 0, 0);
      bitmap.close();
    },
  };

  /** Appelle une méthode du lecteur, dans le Worker. Les erreurs vont dans `state.error`. */
  function player<T = unknown>(method: PlayerMethod, ...args: unknown[]): Promise<T | undefined> {
    return (async () => (await rpc().player(method, args)) as T)().catch((error: Error) => {
      state.error = error.message;
      return undefined;
    });
  }

  async function init(): Promise<void> {
    if (typeof Worker === "undefined") throw new Error("ce navigateur ne sait pas lancer de Worker");
    worker = new Worker(new URL("./emulator.worker.ts", import.meta.url), { type: "module" });
    remote = Comlink.wrap<WorkerApi>(worker);
    worker.onerror = (event) => {
      state.error = event.message || "erreur de l'émulateur";
    };
    const wasm = new URL(wasmUrl, location.href).href;
    await rpc().init(wasm, Comlink.proxy(pageApi));
    state.ready = true;
    if (canvas) void player("draw");
  }

  /** Les gestes tactiles se font sur l'écran : on y branche le mapping dès qu'on a la cible. */
  function attach(element: HTMLCanvasElement, gestures: HTMLElement = element): void {
    canvas = element;
    if (state.ready) void player("draw"); // l'écran déjà dessiné par le Worker va dans ce canvas
    if (!detachPointer) detachPointer = input.attachPointer(gestures);
  }

  /** Envoie une requête au module ; les erreurs sont gardées dans `state.error`. */
  /** Un appareil neuf remplace l'ancien : ce qui se compare d'un appareil à l'autre (la « génération » des fichiers) repart de zéro. */
  const newDevice = <T>(done: Promise<T>): Promise<T> => done.finally(() => void state.deviceSeq++);

  /** Appelé après toute modification des fichiers de la calculatrice par l'utilisateur : la conservation les copie tout de suite. */
  let filesChanged: () => void = () => {};
  const onFilesChanged = (callback: () => void): void => void (filesChanged = callback);
  const WRITES = new Set(["fs_write", "fs_remove", "fs_rename", "fs_import"]);

  const call = (request: Request): Promise<Reply> =>
    (async () => await rpc().request(request, false))().then(
      (reply) => {
        if (reply.ok && WRITES.has(String(request.cmd))) filesChanged();
        return reply;
      },
      (error: Error) => ({ ok: false, error: error.message }),
    );

  /** Comme `call`, mais une erreur n'est pas affichée : pour les lectures périodiques des vues. */
  const quiet = (request: Request): Promise<Reply> =>
    (async () => await rpc().request(request, true))().catch((error: Error) => ({ ok: false, error: error.message }));

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
  const setKept = (document: unknown): void => void remote?.kept(document);

  /** L'écran en image PNG (le Worker le dessine : la page n'a pas ses pixels). */
  const capture = (): Promise<Blob | null> => (async () => await rpc().capture())().catch(() => null);

  /** Commence l'enregistrement de l'écran en GIF ; `false` si c'est impossible. */
  const startRecording = (): Promise<boolean> => (async () => (await rpc().record("start")) as boolean)().catch(() => false);
  /** Termine l'enregistrement et rend le fichier GIF (`null` s'il est vide). */
  const stopRecording = (): Promise<Uint8Array | null> => (async () => (await rpc().record("stop")) as Uint8Array | null)().catch(() => null);

  return {
    state,
    init,
    call,
    quiet,
    onFilesChanged,
    attach,
    keymap,
    setKeyMap,
    attachKeyboard,
    press,
    setKept,
    capture,
    startRecording,
    stopRecording,
    load: (nwa: Uint8Array, name: string, data: Uint8Array | null = null, flash: unknown = null) => newDevice(player<boolean>("load", nwa, name, data, flash)),
    restart: () => newDevice(player<boolean>("restart")),
    stepFrame: () => player<boolean>("stepFrame"),
    runWithoutData: () => newDevice(player<boolean>("runWithoutData")),
    changeModel: (model: Model, firmware?: Firmware) => newDevice(player("changeModel", model, firmware)),
    setSpeed: (speed: SpeedMode) => player("setSpeed", speed),
    setCustomMhz: (mhz: number) => player("setCustomMhz", mhz),
    start: () => player("start"),
    stop: () => player("stop"),
    bump: () => player("bump"),
    /** Redessine l'écran de l'appareil (après une remontée dans le temps, par exemple). */
    draw: () => player("draw"),
    dump: (parts: readonly string[]) => player<unknown>("dump", parts),
    restore: (document: unknown) => newDevice(player<boolean>("restore", document)),
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
