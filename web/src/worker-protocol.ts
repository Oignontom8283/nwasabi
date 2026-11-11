// Les messages entre la page et le fil d'exécution qui fait tourner l'émulation (`emulator.worker.ts`).
// L'émulation occupe un processeur presque entier quand l'application calcule sans attendre : sur le fil de la page,
// elle figerait toute l'interface. Elle tourne donc dans un Worker ; la page lui parle par ces messages et reçoit en
// retour les changements de l'état du lecteur. L'écran est dessiné par le Worker lui-même (OffscreenCanvas), puis envoyé à la page image par image.

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

/** Ce que la page offre au Worker : il l'appelle pour lui donner les changements d'état et l'écran. */
export interface PageApi {
  /** Les champs de l'état du lecteur qui ont changé depuis le dernier appel. */
  state(patch: StatePatch): void;
  /** L'écran, à dessiner dans le canvas de la page (le Worker le transfère : la page doit le fermer). */
  screen(bitmap: ImageBitmap): void;
}

/** Ce que le Worker offre à la page : des fonctions qu'elle appelle comme les siennes (`comlink` s'occupe des messages). */
export interface WorkerApi {
  /** Charge le module WebAssembly et prépare le lecteur ; `page` reçoit ensuite l'état et l'écran. */
  init(wasmUrl: string, page: PageApi): Promise<true>;
  /** Appelle une méthode du lecteur. */
  player(method: PlayerMethod, args: unknown[]): unknown;
  /** Envoie une requête au module ; `quiet` : une erreur n'est pas affichée par le lecteur. */
  request(request: Request, quiet: boolean): Reply;
  /** Les fichiers à remettre dans chaque appareil neuf (`.nfs`), ou `null`. */
  kept(document: unknown | null): void;
  /** L'écran en image PNG. */
  capture(): Promise<Blob | null>;
  /** Enregistrement de l'écran en GIF : `start` le commence (rend `false` si c'est impossible), `stop` le termine et rend le fichier. */
  record(action: "start" | "stop"): boolean | Uint8Array | null;
}

export type { Reply };
