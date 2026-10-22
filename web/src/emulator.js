// Lien entre l'émulateur (@nwasabi/js) et l'interface Vue : le lecteur modifie un état réactif en place.

import { reactive } from "vue";
import { Player } from "@nwasabi/js";
// Le module WebAssembly construit localement (voir vite.config.js) : on n'utilise pas celui de @nwasabi/wasm.
import wasmUrl from "nwasabi.wasm?url";

export function useEmulator() {
  const state = reactive({ ...Player.initialState(), ready: false });
  let player = null;
  // Le canvas est donné avant la fin du chargement du module : on le garde pour l'attacher ensuite.
  let canvas = null;

  async function init() {
    player = await Player.create(await fetch(wasmUrl), { state });
    if (canvas) player.attach(canvas);
    state.ready = true;
  }

  function attach(element) {
    canvas = element;
    player?.attach(element);
  }

  /** Envoie une requête au module ; les erreurs sont gardées dans `state.error`. */
  function call(request) {
    if (!player) return { ok: false, error: "module WebAssembly pas encore chargé" };
    return player.call(request);
  }

  const through = (method) => (...args) => player?.[method](...args);

  return {
    state,
    init,
    call,
    attach,
    draw: through("draw"),
    load: through("load"),
    restart: through("restart"),
    changeModel: through("changeModel"),
    start: through("start"),
    stop: through("stop"),
    key: through("key"),
    bump: through("bump"),
  };
}
