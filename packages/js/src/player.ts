// Boucle d'exécution de l'émulateur dans un navigateur (ou tout environnement qui a `requestAnimationFrame`).
//
// Le cœur ne dort jamais : c'est cette boucle qui fait passer le temps. À chaque image de l'écran du
// navigateur (~60 Hz), on exécute l'application jusqu'à sa prochaine image (`wait_for_vblank`) et on
// dessine l'écran. Elle gère aussi les touches (un appui plus court qu'une image est prolongé) et la
// reprise des applications qui attendent une touche.

import { Nwasabi, toBase64, SCREEN_HEIGHT, SCREEN_WIDTH } from "./nwasabi.js";
import type { Reply, Request, WasmSource } from "./nwasabi.js";
import type { KeyName } from "./keys.js";

export type Model = "n0110" | "n0115" | "n0120";
/** Disposition du firmware : `"24"` (appareils relevés, Epsilon 23–24) ou `"26.4.0"`. */
export type Firmware = "24" | "26.4.0";

/** Pourquoi l'exécution s'est arrêtée la dernière fois (champ `stop` de la réponse `run`). */
export interface StopInfo {
  kind: string;
  [field: string]: unknown;
}

/** L'état que lit l'interface. Il peut être « réactif » (Vue, Svelte…) : le lecteur le modifie en place. */
export interface PlayerState {
  loaded: boolean;
  running: boolean;
  model: Model;
  firmware: Firmware;
  appName: string;
  stop: StopInfo | null;
  frames: number;
  instructions: number;
  error: string;
  stdout: string;
  /** Augmente quand les vues dérivées (fichiers, mémoire…) doivent se rafraîchir. */
  revision: number;
}

export interface PlayerOptions {
  /** État à utiliser (par exemple un objet réactif) ; sinon un objet ordinaire. */
  state?: PlayerState;
  /** Instructions au plus par image du navigateur (défaut : 2 millions). */
  slice?: number;
}

/** Durée d'une image de la calculatrice (60 Hz), en millisecondes. */
const FRAME_MS = 1000 / 60;
/** Plus de temps que cela entre deux images (onglet caché…) compte comme une pause : l'application ne le voit pas. */
const MAX_ELAPSED_MS = 250;
/** Garde-fou : nombre maximal d'exécutions successives dans une même image du navigateur. */
const MAX_RUNS_PER_TICK = 1000;

/** Instructions au plus par image du navigateur (au-delà, on rend la main pour ne pas figer la page). */
const DEFAULT_SLICE = 2_000_000;

export class Player {
  readonly nwasabi: Nwasabi;
  readonly state: PlayerState;
  private readonly slice: number;
  private canvas: HTMLCanvasElement | null = null;
  private image: ImageData | null = null;
  private last = 0;
  private frame = 0;
  private nwa: Uint8Array | null = null;
  private data: Uint8Array | null = null;
  private readonly pressed = new Set<KeyName>();
  // Une touche relâchée avant que l'application ait pu la lire (un appui plus court qu'une image)
  // reste enfoncée jusqu'à l'image suivante.
  private readonly sampled = new Set<KeyName>();
  private readonly deferred = new Set<KeyName>();

  /** L'état de départ d'un lecteur (à rendre réactif avant de le donner à `create`, si besoin). */
  static initialState(): PlayerState {
    return {
      loaded: false,
      running: false,
      model: "n0110",
      firmware: "24",
      appName: "",
      stop: null,
      frames: 0,
      instructions: 0,
      error: "",
      stdout: "",
      revision: 0,
    };
  }

  private constructor(nwasabi: Nwasabi, options: PlayerOptions) {
    this.nwasabi = nwasabi;
    this.state = options.state ?? Player.initialState();
    this.slice = options.slice ?? DEFAULT_SLICE;
  }

  /** Charge le module WebAssembly (à fournir, voir `Nwasabi.load`) et crée un appareil vide. */
  static async create(wasm: WasmSource, options: PlayerOptions = {}): Promise<Player> {
    const player = new Player(await Nwasabi.load(wasm), options);
    player.nwasabi.call({
      cmd: "new",
      model: player.state.model,
      firmware: player.state.firmware,
      clock: "host",
    });
    return player;
  }

  /** Envoie une requête au module ; les erreurs sont gardées dans `state.error`. */
  call(request: Request): Reply {
    const reply = this.nwasabi.request(request);
    if (!reply.ok) this.state.error = reply.error ?? "erreur inconnue";
    return reply;
  }

  /** Dessine l'écran dans ce canvas (320×240). */
  attach(canvas: HTMLCanvasElement): void {
    this.canvas = canvas;
    this.image = canvas.getContext("2d")?.createImageData(SCREEN_WIDTH, SCREEN_HEIGHT) ?? null;
    this.draw();
  }

  /** Recopie l'écran de l'appareil dans le canvas. */
  draw(): void {
    const context = this.canvas?.getContext("2d");
    if (context && this.image && this.nwasabi.drawTo(this.image)) {
      context.putImageData(this.image, 0, 0);
    }
  }

  /** Signale que les vues dérivées doivent se rafraîchir. */
  bump(): void {
    this.state.revision++;
  }

  /** Charge une application (et ses données externes éventuelles) puis la lance. */
  load(nwa: Uint8Array, name: string, data: Uint8Array | null = null): boolean {
    this.nwa = nwa;
    this.data = data;
    this.state.appName = name;
    return this.restart();
  }

  /** Relance l'application chargée depuis le début. */
  restart(): boolean {
    if (!this.nwa) return false;
    this.stop();
    const state = this.state;
    state.error = "";
    state.stdout = "";
    state.frames = 0;
    this.pressed.clear();
    this.sampled.clear();
    this.deferred.clear();
    const request: Request = {
      cmd: "load",
      model: state.model,
      firmware: state.firmware,
      clock: "host", // le temps de l'application est celui que ce lecteur lui donne, jamais plus
      nwa_base64: toBase64(this.nwa),
    };
    if (this.data) request.data_base64 = toBase64(this.data);
    let reply = this.call(request);
    if (!reply.ok && !this.data && /données externes|external/i.test(reply.error ?? "")) {
      // L'application réclame des données externes : un jeu vide permet de la lancer quand même.
      state.error = "";
      reply = this.call({ ...request, data_base64: "" });
    }
    state.loaded = reply.ok;
    this.draw();
    this.bump();
    if (reply.ok) this.start();
    return reply.ok;
  }

  /** Change le modèle (et le firmware) ; relance l'application chargée, s'il y en a une. */
  changeModel(model: Model, firmware: Firmware = this.state.firmware): void {
    this.state.model = model;
    this.state.firmware = firmware;
    if (this.nwa) {
      this.restart();
    } else {
      this.call({ cmd: "new", model, firmware, clock: "host" });
      this.bump();
    }
  }

  start(): void {
    if (!this.state.loaded || this.state.running) return;
    this.state.running = true;
    this.last = performance.now();
    this.frame = requestAnimationFrame((now) => this.tick(now));
  }

  stop(): void {
    this.state.running = false;
    cancelAnimationFrame(this.frame);
  }

  private advance(ms: number): void {
    if (ms > 0) this.call({ cmd: "advance", us: Math.round(ms * 1000) });
  }

  /**
   * Une image du navigateur. L'application ne voit passer que le temps réellement écoulé depuis la
   * dernière image, jamais plus : ce temps est un « budget » qu'elle consomme en dormant ou en
   * attendant une touche, et dont le reste lui est donné à la fin. En pause, ou si l'onglet est
   * caché, aucun temps ne lui est donné : pour elle, le temps s'arrête.
   */
  private tick(now: number): void {
    const state = this.state;
    if (!state.running) return;
    // Un écran plus rapide que 60 Hz : on saute des images pour garder la cadence de la calculatrice.
    if (now - this.last < FRAME_MS - 1) {
      this.frame = requestAnimationFrame((time) => this.tick(time));
      return;
    }
    let budget = Math.max(0, Math.min(now - this.last, MAX_ELAPSED_MS));
    this.last = now;
    for (let runs = 0; runs < MAX_RUNS_PER_TICK && state.running; runs++) {
      const reply = this.call({ cmd: "run", instructions: this.slice, frames: 1 });
      if (!reply.ok) return this.stop();
      for (const name of this.pressed) this.sampled.add(name);
      for (const name of [...this.deferred]) {
        this.deferred.delete(name);
        this.release(name);
      }
      const stop = reply.stop as StopInfo;
      state.stop = stop;
      state.instructions += reply.instructions as number;
      state.frames += reply.frames as number;
      if (stop.kind === "sleep") {
        // L'application dort : le temps passe pour elle, dans la limite du budget.
        const wanted = (stop.ms as number) ?? 0;
        const step = Math.min(wanted, budget);
        this.advance(step);
        budget -= step;
        if (budget > 0) continue;
        break;
      }
      this.advance(budget);
      budget = 0;
      if (!["frame", "wait_event", "budget"].includes(stop.kind)) {
        state.running = false; // fin, faute, point d'observation, Home…
      }
      break;
    }
    this.draw();
    if (state.frames % 10 === 0 || !state.running) {
      const out = this.call({ cmd: "stdout" });
      state.stdout = (out.text as string | undefined) ?? state.stdout;
    }
    if (!state.running) this.bump();
    else this.frame = requestAnimationFrame((time) => this.tick(time));
  }

  private release(name: KeyName): void {
    this.pressed.delete(name);
    this.sampled.delete(name);
    this.call({ cmd: "key", key: name, action: "release" });
  }

  /** Appuie sur une touche (`down`) ou la relâche. */
  key(name: KeyName, down: boolean): void {
    if (!this.state.loaded) return;
    if (down === this.pressed.has(name)) return;
    if (down) {
      this.pressed.add(name);
      this.deferred.delete(name);
      this.call({ cmd: "key", key: name, action: "press" });
      if (!this.state.running) this.start();
    } else if (this.sampled.has(name)) {
      this.release(name);
    } else {
      this.deferred.add(name);
    }
  }
}
