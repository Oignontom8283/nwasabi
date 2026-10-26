// Le Worker qui fait tourner l'émulation (voir `worker-protocol.ts`). Il possède le lecteur de @nwasabi/js, le module
// WebAssembly et l'écran (un OffscreenCanvas) ; la page ne voit de lui que ses réponses et les changements d'état.

import { Player } from "@nwasabi/js";
import type { PlayerState } from "@nwasabi/js";
import type { FromWorker, PlayerMethod, ToWorker } from "./worker-protocol.js";

const scope = self as unknown as {
  postMessage(message: FromWorker, transfer?: Transferable[]): void;
  onmessage: ((event: MessageEvent<ToWorker>) => void) | null;
};

let player: Player | null = null;
let canvas: OffscreenCanvas | null = null;
let kept: unknown = null;
let sent: Record<string, unknown> = {};

/** Envoie à la page les champs de l'état qui ont changé. */
function flush(): void {
  if (!player) return;
  const state = player.state as unknown as Record<string, unknown>;
  state.timeMs = player.time();
  const patch: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(state)) {
    if (key === "keymapName") continue; // le mapping des touches vit dans la page
    const before = sent[key];
    const same = typeof value === "object" && value !== null ? JSON.stringify(value) === JSON.stringify(before) : Object.is(value, before);
    if (!same) patch[key] = value;
  }
  if (Object.keys(patch).length === 0) return;
  sent = { ...sent, ...patch };
  scope.postMessage({ kind: "state", patch: patch as Partial<PlayerState> });
}

/** Après la création d'un appareil neuf, on y remet les fichiers conservés par la page, avant la première image. */
function restoreKept(): void {
  if (kept && player?.state.loaded) player.nwasabi.request({ cmd: "fs_import", nfs: kept, replace: true });
}

const NEW_DEVICE: ReadonlySet<PlayerMethod> = new Set(["load", "restart", "runWithoutData", "changeModel"]);

async function handle(message: ToWorker): Promise<void> {
  switch (message.kind) {
    case "init": {
      try {
        const wasm = await fetch(message.wasmUrl);
        // L'écran : on laisse le lecteur dessiner lui-même dans l'OffscreenCanvas, sans repasser par la page.
        canvas = message.canvas;
        player = await Player.create(wasm, { slice: message.slice });
        if (canvas) player.attach(canvas as never);
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
