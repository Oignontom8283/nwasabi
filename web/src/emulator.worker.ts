// Le Worker qui fait tourner l'émulation (voir `worker-protocol.ts`). Il possède le lecteur de @nwasabi/js, le module
// WebAssembly et l'écran (un OffscreenCanvas) ; la page ne voit de lui que ses réponses et les changements d'état.

import { Player } from "@nwasabi/js";
import type { PlayerState, Scheduler } from "@nwasabi/js";
import type { FromWorker, PlayerMethod, ToWorker } from "./worker-protocol.js";

const scope = self as unknown as {
  postMessage(message: FromWorker, transfer?: Transferable[]): void;
  onmessage: ((event: MessageEvent<ToWorker>) => void) | null;
};

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

let player: Player | null = null;
let canvas: OffscreenCanvas | null = null;
let kept: unknown = null;
let sent: Record<string, unknown> = {};

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

/** Envoie à la page les champs de l'état qui ont changé. */
function flush(): void {
  if (!player) return;
  const state = player.state as unknown as Record<string, unknown>;
  state.timeMs = player.time();
  const patch: Record<string, unknown> = {};
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
  scope.postMessage({ kind: "state", patch: patch as Partial<PlayerState> });
}

/** Dans tout appareil neuf (avec ou sans application), on remet les fichiers conservés par la page, avant la première image. */
function restoreKept(): void {
  if (kept && player) player.nwasabi.request({ cmd: "fs_import", nfs: kept, replace: true });
}

const NEW_DEVICE: ReadonlySet<PlayerMethod> = new Set(["load", "restart", "runWithoutData", "changeModel"]);

async function handle(message: ToWorker): Promise<void> {
  switch (message.kind) {
    case "init": {
      try {
        const wasm = await fetch(message.wasmUrl);
        // L'écran : on laisse le lecteur dessiner lui-même dans l'OffscreenCanvas, sans repasser par la page.
        canvas = message.canvas;
        player = await Player.create(wasm, { scheduler, slice: SLICE, maxWallMs: FRAME_MS });
        if (canvas) player.attach(canvas as never);
        restoreKept();
        player.onFrame(() => flush());
        setInterval(flush, 250); // le temps de l'application continue de défiler même sans nouvelle image
        flush();
        scope.postMessage({ kind: "reply", id: message.id, result: true });
      } catch (error) {
        scope.postMessage({ kind: "failure", id: message.id, message: (error as Error).message });
      }
      return;
    }
    case "kept":
      kept = message.document;
      restoreKept();
      flush();
      return;
    case "request": {
      if (!player) return void scope.postMessage({ kind: "failure", id: message.id, message: "émulateur pas encore prêt" });
      const result = message.quiet ? player.nwasabi.request(message.request) : player.call(message.request);
      scope.postMessage({ kind: "reply", id: message.id, result });
      flush();
      return;
    }
    case "player": {
      if (!player) return void scope.postMessage({ kind: "failure", id: message.id, message: "émulateur pas encore prêt" });
      try {
        const method = (player as unknown as Record<string, (...args: unknown[]) => unknown>)[message.method];
        const result = method?.apply(player, message.args);
        if (NEW_DEVICE.has(message.method)) restoreKept();
        scope.postMessage({ kind: "reply", id: message.id, result });
      } catch (error) {
        scope.postMessage({ kind: "failure", id: message.id, message: (error as Error).message });
      }
      flush();
      return;
    }
    case "capture": {
      const blob = canvas ? await canvas.convertToBlob({ type: "image/png" }).catch(() => null) : null;
      scope.postMessage({ kind: "reply", id: message.id, result: blob });
      return;
    }
  }
}

scope.onmessage = (event) => void handle(event.data);
