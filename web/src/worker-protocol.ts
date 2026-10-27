// Les messages entre la page et le fil d'exécution qui fait tourner l'émulation (`emulator.worker.ts`).
// L'émulation occupe un processeur presque entier quand l'application calcule sans attendre : sur le fil de la page,
// elle figerait toute l'interface. Elle tourne donc dans un Worker ; la page lui parle par ces messages et reçoit en
// retour les changements de l'état du lecteur. L'écran est dessiné par le Worker lui-même (OffscreenCanvas).

import type { PlayerState, Reply, Request } from "@nwasabi/js";

/** Les champs de l'état du lecteur qui ont changé, plus les mesures de vitesse que le Worker ajoute. */
export type StatePatch = Partial<PlayerState> & {
  /** Millions d'instructions émulées par seconde, mesurés sur la dernière demi-seconde (0 en pause). */
  mips?: number;
  /** L'émulation n'a pas pu suivre le modèle émulé pendant la dernière demi-seconde. */
  slow?: boolean;
};

/** Les méthodes du lecteur que la page peut appeler. */
export type PlayerMethod =
  | "load"
  | "restart"
  | "stepFrame"
  | "runWithoutData"
  | "changeModel"
  | "setSpeed"
  | "setCustomMhz"
  | "start"
  | "stop"
  | "key"
  | "bump"
  | "draw"
  | "dump"
  | "restore";

/** Ce que la page envoie. */
export type ToWorker =
  | { kind: "init"; id: number; wasmUrl: string; canvas: OffscreenCanvas | null }
  | { kind: "player"; id: number; method: PlayerMethod; args: unknown[] }
  | { kind: "request"; id: number; request: Request; quiet: boolean }
  /** Les fichiers à remettre dans chaque appareil neuf (`.nfs`), ou `null`. */
  | { kind: "kept"; document: unknown | null }
  | { kind: "capture"; id: number }
  /** Enregistrement de l'écran en GIF : `start` le commence, `stop` le termine et rend le fichier (`Uint8Array`). */
  | { kind: "record"; id: number; action: "start" | "stop" };

/** Ce que le Worker répond. */
export type FromWorker =
  | { kind: "reply"; id: number; result: unknown }
  | { kind: "failure"; id: number; message: string }
  /** Les champs de l'état du lecteur qui ont changé depuis le dernier message. */
  | { kind: "state"; patch: StatePatch };

export type { Reply };
