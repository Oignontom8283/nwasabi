// Lien entre l'émulateur (@nwasabi/js) et l'interface Vue : le lecteur modifie un état réactif en place.

import { inject, reactive } from "vue";
import { Player } from "@nwasabi/js";
import type { KeyMap, KeyName, Reply, Request } from "@nwasabi/js";
// Le module WebAssembly construit localement (voir vite.config.ts) : on n'utilise pas celui de @nwasabi/wasm.
import wasmUrl from "nwasabi.wasm?url";

export function useEmulator() {
  // `keymapVersion` change à chaque nouveau mapping des touches : les vues qui l'affichent le suivent.
  const state = reactive({ ...Player.initialState(), ready: false, keymapVersion: 0 });
  let player: Player | null = null;
  // Le canvas est donné avant la fin du chargement du module : on le garde pour l'attacher ensuite.
  let canvas: HTMLCanvasElement | null = null;
  let gestureTarget: HTMLElement | null = null;
  let detachPointer: (() => void) | null = null;

  /** Les gestes tactiles se font sur l'écran : on y branche le lecteur dès qu'on a les deux. */
  function wirePointer(): void {
    if (!player || !gestureTarget || detachPointer) return;
    detachPointer = player.input.attachPointer(gestureTarget);
  }

  async function init(): Promise<void> {
    player = await Player.create(await fetch(wasmUrl), { state });
    if (canvas) player.attach(canvas);
    wirePointer();
    state.ready = true;
  }

  function attach(element: HTMLCanvasElement, gestures: HTMLElement = element): void {
    canvas = element;
    gestureTarget = gestures;
    player?.attach(element);
    wirePointer();
  }

  const notReady: Reply = { ok: false, error: "module WebAssembly pas encore chargé" };

  /** Envoie une requête au module ; les erreurs sont gardées dans `state.error`. */
  function call(request: Request): Reply {
    return player ? player.call(request) : notReady;
  }

  /** Comme `call`, mais une erreur n'est pas affichée : pour les lectures périodiques des vues. */
  function quiet(request: Request): Reply {
    return player ? player.nwasabi.request(request) : notReady;
  }

  /** Le mapping des touches en vigueur. */
  const keymap = (): KeyMap | undefined => player?.input.keymap;

  function setKeyMap(map: KeyMap): void {
    player?.setKeyMap(map);
    state.keymapVersion++;
  }

  /** Branche le clavier de l'ordinateur (voir `InputMapper.attachKeyboard`) ; rend la fonction qui débranche. */
  const attachKeyboard = (target: Window, ignore?: (event: KeyboardEvent) => boolean): (() => void) => player?.input.attachKeyboard(target, ignore) ?? (() => {});

  /** Une touche de la calculatrice enfoncée ou relâchée par un clic sur l'écran, hors du mapping. */
  const press = (name: KeyName, down: boolean): void => player?.key(name, down);

  /** Une méthode du lecteur, sans effet tant que le module n'est pas chargé. */
  function through<K extends "draw" | "setSpeed" | "setCustomMhz" | "start" | "stop" | "key" | "bump" | "dump" | "restore">(method: K) {
    return (...args: Parameters<Player[K]>): ReturnType<Player[K]> | undefined => {
      const fn = player?.[method] as ((...a: unknown[]) => ReturnType<Player[K]>) | undefined;
      return fn?.apply(player, args);
    };
  }

  // Chaque chargement ou relance crée un appareil neuf, avec des fichiers vides : ceux qui veulent y remettre quelque
  // chose (les fichiers conservés dans le navigateur) s'inscrivent ici ; ils sont appelés avant la première image.
  const deviceHooks: Array<() => void> = [];
  const onDevice = (hook: () => void): void => void deviceHooks.push(hook);
  const afterDevice = () => {
    if (state.loaded) for (const hook of deviceHooks) hook();
  };

  const load = (nwa: Uint8Array, name: string, data: Uint8Array | null = null): boolean => {
    const ok = player?.load(nwa, name, data) ?? false;
    if (ok) afterDevice();
    return ok;
  };
  const restart = (): boolean => {
    const ok = player?.restart() ?? false;
    if (ok) afterDevice();
    return ok;
  };
  const runWithoutData = (): boolean => {
    const ok = player?.runWithoutData() ?? false;
    if (ok) afterDevice();
    return ok;
  };
  const changeModel = (...args: Parameters<Player["changeModel"]>): void => {
    player?.changeModel(...args);
    afterDevice();
  };

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
    draw: through("draw"),
    onDevice,
    load,
    restart,
    runWithoutData,
    changeModel,
    setSpeed: through("setSpeed"),
    setCustomMhz: through("setCustomMhz"),
    start: through("start"),
    stop: through("stop"),
    key: through("key"),
    bump: through("bump"),
    dump: through("dump"),
    restore: through("restore"),
    time: () => player?.time() ?? 0,
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
