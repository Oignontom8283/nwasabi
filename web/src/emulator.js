// Lien entre l'émulateur (@nwasabi/js) et l'interface Vue : le lecteur modifie un état réactif en place.

import { reactive } from "vue";
import { Player } from "@nwasabi/js";
// Le module WebAssembly construit localement (voir vite.config.js) : on n'utilise pas celui de @nwasabi/wasm.
import wasmUrl from "nwasabi.wasm?url";

export function useEmulator() {
  // `keymapVersion` change à chaque nouveau mapping des touches : les vues qui l'affichent le suivent.
  const state = reactive({ ...Player.initialState(), ready: false, keymapVersion: 0 });
  let player = null;
  // Le canvas est donné avant la fin du chargement du module : on le garde pour l'attacher ensuite.
  let canvas = null;
  let detachPointer = null;

  /** Les gestes tactiles se font sur l'écran : on y branche le lecteur dès qu'on a les deux. */
  function wirePointer() {
    if (!player || !canvas || detachPointer) return;
    detachPointer = player.input.attachPointer(canvas);
  }

  async function init() {
    player = await Player.create(await fetch(wasmUrl), { state });
    if (canvas) player.attach(canvas);
    wirePointer();
    state.ready = true;
  }

  function attach(element) {
    canvas = element;
    player?.attach(element);
    wirePointer();
  }

  /** Envoie une requête au module ; les erreurs sont gardées dans `state.error`. */
  function call(request) {
    if (!player) return { ok: false, error: "module WebAssembly pas encore chargé" };
    return player.call(request);
  }

  /** Le mapping des touches en vigueur. */
  const keymap = () => player?.input.keymap;

  function setKeyMap(map) {
    player?.setKeyMap(map);
    state.keymapVersion++;
  }

  /** Branche le clavier de l'ordinateur (voir `InputMapper.attachKeyboard`) ; rend la fonction qui débranche. */
  const attachKeyboard = (target, ignore) => player?.input.attachKeyboard(target, ignore) ?? (() => {});

  const through = (method) => (...args) => player?.[method](...args);

  return {
    state,
    init,
    call,
    attach,
    keymap,
    setKeyMap,
    attachKeyboard,
    draw: through("draw"),
    load: through("load"),
    restart: through("restart"),
    changeModel: through("changeModel"),
    setSpeed: through("setSpeed"),
    setCustomMhz: through("setCustomMhz"),
    start: through("start"),
    stop: through("stop"),
    key: through("key"),
    bump: through("bump"),
  };
}
