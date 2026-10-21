// Boucle d'exécution de l'émulateur dans le navigateur.
//
// Le cœur ne dort jamais : c'est cette boucle qui fait passer le temps. À chaque image de
// l'écran du navigateur (~60 Hz), on exécute l'application jusqu'à sa prochaine image
// (`wait_for_vblank`) et on dessine l'écran.

import { reactive, shallowRef } from "vue";
import { Nwasabi, toBase64 } from "./nwasabi.js";
import wasmUrl from "./nwasabi.wasm?url";

/** Instructions au plus par image du navigateur (au-delà, on rend la main pour ne pas figer la page). */
const SLICE = 2_000_000;

export function useEmulator() {
  const emu = shallowRef(null);
  const state = reactive({
    ready: false,
    loaded: false,
    running: false,
    model: "n0110",
    appName: "",
    stop: null, // dernière raison d'arrêt
    frames: 0,
    instructions: 0,
    error: "",
    stdout: "",
    revision: 0, // augmente quand les vues (fichiers, mémoire…) doivent se rafraîchir
  });
  let canvas = null;
  let image = null;
  let last = 0;
  let raf = 0;
  let nwa = null;
  let data = null;
  const pressed = new Set();
  // Une touche relâchée avant que l'application ait pu la lire (un appui plus court qu'une image)
  // reste enfoncée jusqu'à l'image suivante.
  const sampled = new Set();
  const deferred = new Set();

  async function init() {
    emu.value = await Nwasabi.load(await fetch(wasmUrl));
    emu.value.call({ cmd: "new", model: state.model });
    state.ready = true;
  }

  /** Envoie une requête au module ; les erreurs sont gardées dans `state.error`. */
  function call(request) {
    if (!emu.value) return { ok: false, error: "module WebAssembly pas encore chargé" };
    const reply = emu.value.request(request);
    if (!reply.ok) state.error = reply.error;
    return reply;
  }

  function attach(element) {
    canvas = element;
    image = canvas.getContext("2d").createImageData(320, 240);
    draw();
  }

  function draw() {
    if (canvas && emu.value && emu.value.drawTo(image)) {
      canvas.getContext("2d").putImageData(image, 0, 0);
    }
  }

  function bump() {
    state.revision++;
  }

  async function load(bytes, name, dataBytes = null) {
    nwa = bytes;
    data = dataBytes;
    state.appName = name;
    return restart();
  }

  function restart() {
    stop();
    state.error = "";
    state.stdout = "";
    state.frames = 0;
    pressed.clear();
    sampled.clear();
    deferred.clear();
    const request = { cmd: "load", model: state.model, nwa_base64: toBase64(nwa) };
    if (data) request.data_base64 = toBase64(data);
    let reply = call(request);
    if (!reply.ok && !data && /données externes|external/i.test(reply.error)) {
      // L'application réclame des données externes : un jeu vide permet de la lancer quand même.
      state.error = "";
      reply = call({ ...request, data_base64: "" });
    }
    state.loaded = reply.ok;
    draw();
    bump();
    if (reply.ok) start();
    return reply.ok;
  }

  function changeModel(model) {
    state.model = model;
    if (nwa) restart();
    else {
      call({ cmd: "new", model });
      bump();
    }
  }

  function start() {
    if (!state.loaded || state.running) return;
    state.running = true;
    last = performance.now();
    raf = requestAnimationFrame(tick);
  }

  function stop() {
    state.running = false;
    cancelAnimationFrame(raf);
  }

  function tick(now) {
    if (!state.running) return;
    const elapsed = Math.max(0, Math.min(now - last, 250));
    last = now;
    const reply = call({ cmd: "run", instructions: SLICE, frames: 1 });
    if (!reply.ok) return stop();
    for (const name of pressed) sampled.add(name);
    for (const name of deferred) {
      deferred.delete(name);
      release(name);
    }
    state.stop = reply.stop;
    state.instructions += reply.instructions;
    state.frames += reply.frames;
    switch (reply.stop.kind) {
      case "wait_event":
        // L'application attend une touche : le temps ne passe que si on le lui donne.
        call({ cmd: "advance", ms: Math.round(elapsed) });
        break;
      case "frame":
      case "sleep":
      case "budget":
        break;
      default: // fin, faute, point d'observation, Home…
        state.running = false;
        break;
    }
    draw();
    if (state.frames % 10 === 0 || !state.running) {
      state.stdout = call({ cmd: "stdout" }).text ?? state.stdout;
    }
    if (!state.running) bump();
    else raf = requestAnimationFrame(tick);
  }

  function release(name) {
    pressed.delete(name);
    sampled.delete(name);
    call({ cmd: "key", key: name, action: "release" });
  }

  function key(name, down) {
    if (!state.loaded) return;
    if (down === pressed.has(name)) return;
    if (down) {
      pressed.add(name);
      deferred.delete(name);
      call({ cmd: "key", key: name, action: "press" });
      if (!state.running) start();
    } else if (sampled.has(name)) {
      release(name);
    } else {
      deferred.add(name);
    }
  }

  return { emu, state, init, call, attach, draw, load, restart, changeModel, start, stop, key, bump };
}
