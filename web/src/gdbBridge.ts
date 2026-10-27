// Le pont GDB côté page : un débogueur (GDB, LLDB, VS Code…) branché sur le serveur Nwasabi (option `--gdb`) pilote l'émulation de cette
// page, celle qu'on voit. L'émulation tourne ici, dans le navigateur : le serveur reçoit les commandes du débogueur, la page vient les
// chercher (une requête qui attend), les exécute et rend la réponse. Rien ne passe tant que l'utilisateur ne l'a pas activé.

import { reactive } from "vue";
import type { Emulator } from "./emulator.js";

export const gdb = reactive({
  /** Le serveur a été lancé avec `--gdb`. */
  available: false,
  /** Port où se branchent les débogueurs. */
  port: 0,
  /** L'utilisateur accepte un débogueur (désactivé au départ, jamais retenu d'une visite à l'autre). */
  enabled: false,
  /** Nombre de débogueurs connectés. */
  clients: 0,
  error: "",
});

/** Commandes qui changent ce que la page affiche (écran, registres, mémoire) : elle se redessine après. */
const CHANGES = new Set(["run", "step", "step_back", "reverse_continue", "history_goto", "cpu_set", "memory_write", "launch", "key", "advance"]);

let running: AbortController | null = null;
let statusTimer: ReturnType<typeof setInterval> | undefined;
let redrawTimer: ReturnType<typeof setTimeout> | undefined;

/** Interroge le serveur : propose-t-il le serveur GDB ? Hors du serveur Nwasabi (site statique), non. */
export async function probeGdb(): Promise<void> {
  try {
    const reply = await fetch("__nwasabi/gdb", { cache: "no-store" });
    const status = reply.ok ? await reply.json() : { enabled: false };
    gdb.available = status.enabled === true;
    gdb.port = Number(status.port ?? 0);
    gdb.clients = Number(status.clients ?? 0);
  } catch {
    gdb.available = false;
  }
}

/** Active ou coupe le pont. Activé, la page arrête sa propre exécution : c'est le débogueur qui la conduit. */
export function setGdb(emulator: Emulator, on: boolean): void {
  if (on === gdb.enabled) return;
  gdb.enabled = on;
  gdb.error = "";
  running?.abort();
  running = null;
  clearInterval(statusTimer);
  if (!on) {
    void fetch("__nwasabi/gdb/detach", { method: "POST" }).catch(() => {});
    return;
  }
  emulator.stop();
  // Le nombre de débogueurs connectés se relit tant que le pont est actif (la bannière de l'onglet Débogage le montre).
  statusTimer = setInterval(() => void probeGdb(), 1500);
  const controller = new AbortController();
  running = controller;
  void loop(emulator, controller.signal);
}

/** Rend la page sans attendre : le dernier relevé n'est pas perdu si le débogueur enchaîne les commandes. */
function redraw(emulator: Emulator): void {
  if (redrawTimer) return;
  redrawTimer = setTimeout(() => {
    redrawTimer = undefined;
    void emulator.draw();
    emulator.bump();
  }, 40);
}

async function loop(emulator: Emulator, signal: AbortSignal): Promise<void> {
  while (!signal.aborted) {
    try {
      const next = await fetch("__nwasabi/gdb/next", { signal });
      if (next.status === 204) continue; // rien à faire : on redemande (le serveur sait ainsi que la page est là)
      if (!next.ok) throw new Error(`serveur : ${next.status}`);
      const { id, request } = (await next.json()) as { id: number; request: string };
      let reply: unknown;
      let cmd = "";
      try {
        const parsed = JSON.parse(request) as { cmd?: string };
        cmd = String(parsed.cmd ?? "");
        reply = await emulator.quiet(parsed as never);
      } catch (error) {
        reply = { ok: false, error: String((error as Error).message ?? error) };
      }
      await fetch("__nwasabi/gdb/answer", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ id, reply: JSON.stringify(reply) }), signal });
      if (CHANGES.has(cmd)) redraw(emulator);
      gdb.error = "";
    } catch (error) {
      if (signal.aborted) return;
      gdb.error = String((error as Error).message ?? error);
      await new Promise((resolve) => setTimeout(resolve, 1500)); // le serveur ne répond plus : on réessaie sans s'emballer
    }
  }
}

// La page qui se ferme prévient le serveur : un débogueur ne doit pas attendre une page partie.
addEventListener("pagehide", () => {
  if (gdb.enabled) navigator.sendBeacon?.("__nwasabi/gdb/detach");
});
