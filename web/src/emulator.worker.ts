// Le Worker qui fait tourner l'émulation (voir `worker-protocol.ts`). Il possède le lecteur de @nwasabi/js, le module
// WebAssembly et l'écran (un OffscreenCanvas) ; la page ne voit de lui que ses réponses et les changements d'état.

import * as Comlink from "comlink";
import { GifRecorder } from "./gif.js";
import { Player } from "@nwasabi/js";
import type { Scheduler } from "@nwasabi/js";
import type { PageApi, PlayerMethod, StatePatch, WorkerApi } from "./worker-protocol.js";

const FRAME_MS = 1000 / 60;
/** Instructions au plus par tranche : entre deux tranches, le Worker traite les messages de la page (touches, requêtes). */
const SLICE = 100_000;

// Le rythme : une image toutes les 1/60 s, mais une application qui calcule sans attendre utilise toute l'image, puis la
// suivante démarre aussitôt (MessageChannel : pas d'attente minimale d'un `setTimeout`). Une application qui attend
// l'image suivante dort jusqu'à elle. Le Worker n'a pas d'interface à ménager : toute l'image est pour l'émulation.
const immediate = new MessageChannel();
const waiting: Array<() => void> = [];
immediate.port1.onmessage = () => waiting.shift()?.();
let lastTick = 0;
const scheduler: Scheduler = {
  now: () => performance.now(),
  request(callback) {
    const token = { cancelled: false, timer: undefined as ReturnType<typeof setTimeout> | undefined };
    const run = () => {
      if (token.cancelled) return;
      lastTick = performance.now();
      callback(lastTick);
    };
    const wait = FRAME_MS - (performance.now() - lastTick) - 0.5;
    if (wait > 1) token.timer = setTimeout(run, wait);
    else {
      waiting.push(run);
      immediate.port2.postMessage(0);
    }
    return token;
  },
  cancel(handle) {
    const token = handle as { cancelled: boolean; timer: ReturnType<typeof setTimeout> | undefined };
    token.cancelled = true;
    clearTimeout(token.timer);
  },
};

/** La page, pour lui donner l'état et l'écran. */
let page: Comlink.Remote<PageApi> | null = null;
let player: Player | null = null;
let canvas: OffscreenCanvas | null = null;
let kept: unknown = null;
let recorder: GifRecorder | null = null;

/** Si un GIF est en cours, y ajoute l'écran tel qu'il est maintenant. */
function record(): void {
  if (!recorder || !canvas) return;
  const context = canvas.getContext("2d") as OffscreenCanvasRenderingContext2D | null;
  if (context) recorder.add(context.getImageData(0, 0, canvas.width, canvas.height).data, performance.now());
}
let sent: Record<string, unknown> = {};

let showing = false;
let showAgain = false;
/** Envoie l'écran à la page. Un canvas ne se partage pas entre fils : on lui envoie une image, et les images qui arrivent pendant l'envoi n'en font qu'une. */
function show(): void {
  if (!canvas) return;
  if (showing) {
    showAgain = true;
    return;
  }
  showing = true;
  createImageBitmap(canvas)
    .then((bitmap) => page?.screen(Comlink.transfer(bitmap, [bitmap])))
    .catch(() => {})
    .finally(() => {
      showing = false;
      if (showAgain) {
        showAgain = false;
        show();
      }
    });
}

/** Deux petits objets simples (l'arrêt, par exemple) ont-ils les mêmes champs ? */
function shallowEqual(a: Record<string, unknown>, b: unknown): boolean {
  if (typeof b !== "object" || b === null) return false;
  const other = b as Record<string, unknown>;
  const keys = Object.keys(a);
  return keys.length === Object.keys(other).length && keys.every((key) => Object.is(a[key], other[key]));
}

/**
 * Les compteurs qui changent à chaque image (images, instructions, temps) ne sont envoyés que dix fois par seconde au plus :
 * la page ne gagne rien à se redessiner soixante fois par seconde pour un compteur. Le reste de l'état part tout de suite.
 */
const COUNTERS = new Set(["frames", "instructions", "timeMs"]);
const COUNTER_MS = 100;
let counterSentAt = 0;

// Vitesse mesurée : instructions exécutées par seconde réelle, relevées toutes les demi-secondes.
const STATS_MS = 500;
let statsAt = 0;
let statsInstructions = 0;
let statsHits = 0;
let statsOut: { mips: number; slow: boolean } | null = null;
function measure(state: Record<string, unknown>): void {
  const now = performance.now();
  if (now - statsAt < STATS_MS) return;
  const running = state.running === true;
  const instructions = Number(state.instructions ?? 0);
  const hits = Number(state.capHits ?? 0);
  statsOut = {
    mips: running && statsAt > 0 ? Math.max(0, Math.round(((instructions - statsInstructions) / (now - statsAt)) / 100) / 10) : 0,
    slow: running && hits !== statsHits,
  };
  statsAt = now;
  statsInstructions = instructions;
  statsHits = hits;
}

/** Envoie à la page les champs de l'état qui ont changé. */
function flush(): void {
  if (!player) return;
  const state = player.state as unknown as Record<string, unknown>;
  state.timeMs = player.time();
  measure(state);
  const patch: Record<string, unknown> = {};
  if (statsOut) {
    for (const [key, value] of Object.entries(statsOut)) if (!Object.is(sent[key], value)) patch[key] = value;
    statsOut = null;
  }
  const counters = performance.now() - counterSentAt >= COUNTER_MS;
  for (const [key, value] of Object.entries(state)) {
    if (COUNTERS.has(key) && !counters) continue;
    if (key === "keymapName") continue; // le mapping des touches vit dans la page
    const before = sent[key];
    const same = typeof value === "object" && value !== null ? shallowEqual(value as Record<string, unknown>, before) : Object.is(value, before);
    if (!same) patch[key] = value;
  }
  if (Object.keys(patch).length === 0) return;
  if (counters) counterSentAt = performance.now();
  sent = { ...sent, ...patch };
  void page?.state(patch as StatePatch);
}

/**
 * Les fichiers de la calculatrice passent d'un appareil à l'autre : charger une autre application, la relancer ou changer de modèle
 * crée un appareil neuf, et il reprend les fichiers de l'ancien (relevés juste avant). Les fichiers conservés par la page ne servent
 * qu'au départ, pour le tout premier appareil.
 */
let carried: unknown = null;
let keptApplied = false;

/** Relève les fichiers de l'appareil actuel, avant qu'un appareil neuf ne le remplace. */
function carryFiles(): void {
  if (!player) return;
  const exported = player.nwasabi.request({ cmd: "fs_export" });
  if (exported.ok) carried = exported.nfs;
}

/** Dans tout appareil neuf (avec ou sans application), on remet les fichiers repris, avant la première image. */
function restoreFiles(): void {
  const document = carried ?? kept;
  if (document && player) player.nwasabi.request({ cmd: "fs_import", nfs: document, replace: true });
}

/**
 * Les points d'arrêt suivent l'application d'un appareil à l'autre : relancer, changer de modèle ou recharger la même application les
 * remet en place avant la première instruction, comme dans un débogueur d'ordinateur. Une autre application les oublie.
 */
let breakpoints: Array<{ address: number; condition?: unknown }> = [];
let breakpointsApp = "";
const BREAK_COMMANDS = new Set(["break", "unbreak", "break_line", "unbreak_line", "break_function"]);

/** Relève les points d'arrêt de l'appareil actuel, après une commande qui les change. */
function noteBreakpoints(): void {
  if (!player) return;
  const reply = player.nwasabi.request({ cmd: "breaks" });
  if (reply.ok) breakpoints = (reply.details as typeof breakpoints) ?? [];
}

/** Dans tout appareil neuf, on remet les points d'arrêt connus. */
function restoreBreakpoints(): void {
  if (!player) return;
  for (const b of breakpoints) player.nwasabi.request({ cmd: "break", address: b.address, condition: b.condition ?? null });
}

const NEW_DEVICE: ReadonlySet<PlayerMethod> = new Set(["load", "restart", "runWithoutData", "changeModel"]);

const NOT_READY = "émulateur pas encore prêt";

const api: WorkerApi = {
  async init(wasmUrl, remote) {
    page = remote as unknown as Comlink.Remote<PageApi>; // ce que la page a donné à `Comlink.proxy` : ses fonctions deviennent asynchrones ici
    const wasm = await fetch(wasmUrl);
    // L'écran : le lecteur dessine dans un canvas du Worker, dont chaque image part ensuite à la page (voir `show`).
    canvas = new OffscreenCanvas(320, 240);
    player = await Player.create(wasm, { scheduler, slice: SLICE, maxWallMs: FRAME_MS });
    player.attach(canvas as never);
    restoreFiles();
    player.onFrame(() => {
      record();
      show();
      flush();
    });
    setInterval(flush, 250); // le temps de l'application continue de défiler même sans nouvelle image
    flush();
    return true;
  },

  kept(document) {
    kept = document;
    // La première fois (au démarrage), les fichiers conservés vont dans l'appareil déjà là ; ensuite, changer le réglage ne doit pas
    // écraser les fichiers de l'appareil en cours avec une copie plus ancienne.
    if (!keptApplied) restoreFiles();
    keptApplied = true;
    flush();
  },

  request(request, quiet) {
    if (!player) throw new Error(NOT_READY);
    try {
      const result = quiet ? player.nwasabi.request(request) : player.call(request);
      if (BREAK_COMMANDS.has(String(request.cmd))) noteBreakpoints();
      return result;
    } finally {
      flush();
    }
  },

  player(name, args) {
    if (!player) throw new Error(NOT_READY);
    try {
      const method = (player as unknown as Record<string, (...args: unknown[]) => unknown>)[name];
      if (NEW_DEVICE.has(name)) carryFiles();
      if (name === "load") {
        // Une autre application : ses points d'arrêt ne sont pas ceux de la précédente.
        const appName = String(args[1] ?? "");
        if (appName !== breakpointsApp) breakpoints = [];
        breakpointsApp = appName;
      }
      const result = method?.apply(player, args);
      if (NEW_DEVICE.has(name)) {
        restoreFiles();
        restoreBreakpoints();
      }
      show();
      return result;
    } finally {
      flush();
    }
  },

  record(action) {
    if (action === "start") {
      recorder = canvas ? new GifRecorder(canvas.width, canvas.height) : null;
      record();
      return recorder !== null;
    }
    const gif = recorder && recorder.frames > 0 ? recorder.finish() : null;
    recorder = null;
    return gif;
  },

  async capture() {
    return canvas ? await canvas.convertToBlob({ type: "image/png" }).catch(() => null) : null;
  },
};

Comlink.expose(api);
