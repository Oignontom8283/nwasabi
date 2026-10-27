// Boucle d'exécution de l'émulateur, dans un navigateur comme dans Node.js.
//
// Rien ici ne suppose le navigateur : le rythme vient d'un `Scheduler` (par défaut `requestAnimationFrame`
// s'il existe, sinon des minuteries à 60 Hz) et l'écran sort sous forme de pixels bruts (`onFrame`,
// `framebuffer()`, `rgba()`) ; dessiner dans un canvas (`attach`) n'est qu'un confort.
//
// Le cœur ne dort jamais : c'est cette boucle qui fait passer le temps. À chaque image (~60 Hz), on exécute l'application jusqu'à sa prochaine image (`wait_for_vblank`) et on
// dessine l'écran. Elle gère aussi les touches (un appui plus court qu'une image est prolongé) et la
// reprise des applications qui attendent une touche.

import { Nwasabi, fromBase64, toBase64, SCREEN_HEIGHT, SCREEN_WIDTH } from "./nwasabi.js";
import type { Reply, Request, RgbaTarget, WasmSource } from "./nwasabi.js";
import type { KeyName } from "./keys.js";
import { InputMapper } from "./input.js";
import { KeyMap } from "./keymap.js";

export type Model = "n0110" | "n0115" | "n0120";
/** Disposition du firmware : `"24"` (appareils relevés, Epsilon 23–24) ou `"26.4.0"`. */
export type Firmware = "24" | "26.4.0";

/**
 * Vitesse du processeur émulé :
 * - `"model"` : la fréquence du modèle (192 MHz sur N0110 et N0115, 540 MHz sur N0120) ;
 * - `"custom"` : la fréquence `customMhz` ;
 * - `"max"` : aucune limite, l'application s'exécute au plus vite (elle reste rythmée par les images
 *   de l'écran : 60 par seconde).
 */
export type SpeedMode = "model" | "custom" | "max";

/** Fréquence personnalisée de départ, en MHz : celle du N0110. */
export const DEFAULT_CUSTOM_MHZ = 192;

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
  /** Vitesse du processeur (voir `SpeedMode`). */
  speed: SpeedMode;
  /** Fréquence en MHz du mode `"custom"` ; ignorée dans les autres modes. */
  customMhz: number;
  /** Fréquence appliquée en ce moment, en MHz ; `null` quand la vitesse n'est pas limitée. Donnée par le module. */
  limitMhz: number | null;
  appName: string;
  stop: StopInfo | null;
  frames: number;
  instructions: number;
  error: string;
  stdout: string;
  /** Temps de simulation écoulé depuis le début de l'application, en millisecondes (mis à jour à chaque image). */
  timeMs: number;
  /** Nom du mapping des touches en vigueur (`null` : sans nom, ou celui par défaut). */
  keymapName: string | null;
  /** L'application demande des données externes qu'on ne lui a pas données (elle n'est pas lancée). */
  needsData: boolean;
  /** Augmente quand les vues dérivées (fichiers, mémoire…) doivent se rafraîchir. */
  revision: number;
  /**
   * Combien de fois l'émulation n'a pas pu suivre le processeur émulé : le temps réel de l'image est épuisé alors que l'application
   * avait encore des instructions à exécuter (elle tourne alors plus lentement que sur la vraie calculatrice). Sans limite de
   * vitesse, il ne bouge pas.
   */
  capHits: number;
}

/** Ce qui rythme le lecteur : l'heure, et « rappelle-moi à la prochaine image ». */
export interface Scheduler {
  /** Heure actuelle en millisecondes (n'importe quelle origine, mais croissante). */
  now(): number;
  /** Demande un appel de `callback(heure)` à la prochaine image. */
  request(callback: (time: number) => void): unknown;
  /** Annule une demande faite par `request`. */
  cancel(handle: unknown): void;
}

/**
 * Le planificateur par défaut : `requestAnimationFrame` quand il existe (navigateur), sinon des
 * minuteries (`setTimeout`) à environ 60 Hz, par exemple dans Node.js.
 */
export function defaultScheduler(): Scheduler {
  const g = globalThis as unknown as {
    requestAnimationFrame?: (callback: (time: number) => void) => unknown;
    cancelAnimationFrame?: (handle: unknown) => void;
    performance?: { now(): number };
  };
  const now = () => (g.performance ? g.performance.now() : Date.now());
  if (typeof g.requestAnimationFrame === "function" && typeof g.cancelAnimationFrame === "function") {
    return {
      now,
      request: (callback) => g.requestAnimationFrame!(callback),
      cancel: (handle) => g.cancelAnimationFrame!(handle),
    };
  }
  return {
    now,
    request: (callback) => setTimeout(() => callback(now()), FRAME_MS),
    cancel: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
  };
}

/** Un canvas 2D, ou tout ce qui lui ressemble (`HTMLCanvasElement`, `OffscreenCanvas`, `node-canvas`…). */
export interface CanvasLike {
  getContext(type: "2d"): {
    createImageData(width: number, height: number): RgbaTarget;
    putImageData(image: never, x: number, y: number): void;
  } | null;
}

/** Une image de l'écran, brute. Les tableaux ne sont valables que pendant l'appel de l'écouteur. */
export interface Frame {
  width: number;
  height: number;
  /** Pixels en RGB565 (16 bits : 5 rouge, 6 vert, 5 bleu), ligne par ligne, tels que l'appareil les a. */
  rgb565: Uint16Array;
  /** Les mêmes pixels en RGBA 8 bits (nouveau tampon à chaque appel). */
  rgba(): Uint8ClampedArray;
  /** Temps de simulation écoulé, en millisecondes. */
  timeMs: number;
}

export interface PlayerOptions {
  /** Ce qui rythme la boucle ; par défaut `defaultScheduler()`. Utile pour les tests ou un autre moteur d'images. */
  scheduler?: Scheduler;
  /** Appelé à chaque image avec les pixels bruts de l'écran (voir aussi `Player.onFrame`). */
  onFrame?: (frame: Frame) => void;
  /** État à utiliser (par exemple un objet réactif) ; sinon un objet ordinaire. */
  state?: PlayerState;
  /** Vitesse du processeur au départ (défaut : `"model"`). */
  speed?: SpeedMode;
  /** Fréquence du mode `"custom"` au départ, en MHz (défaut : `DEFAULT_CUSTOM_MHZ`). */
  customMhz?: number;
  /** Mapping des touches au départ (défaut : `KeyMap.default()`). */
  keymap?: KeyMap;
  /**
   * Temps réel au plus passé à émuler par image, en millisecondes (défaut : 12). Sur le fil de la page, on laisse le reste
   * de l'image à l'interface ; dans un Worker, on peut donner toute l'image à l'émulation.
   */
  maxWallMs?: number;
  /** Instructions au plus par image du navigateur (défaut : 2 millions). */
  slice?: number;
  /**
   * Temps réel maximal donné à l'application pour une seule image, en millisecondes (défaut : 250).
   * Une image très en retard (machine surchargée, débogueur…) est ramenée à cette valeur : ce n'est
   * qu'un garde-fou. La pause (onglet caché, bouton…) est l'affaire de l'application hôte, qui appelle
   * `stop()` et `start()`.
   */
  maxElapsedMs?: number;
}

/** Durée d'une image de la calculatrice (60 Hz), en millisecondes. */
const FRAME_MS = 1000 / 60;
/** Garde-fou par défaut contre une image très en retard (voir `PlayerOptions.maxElapsedMs`). */
const DEFAULT_MAX_ELAPSED_MS = 250;
/** Garde-fou : nombre maximal d'exécutions successives dans une même image du navigateur. */
const MAX_RUNS_PER_TICK = 1000;
/** Temps réel au plus passé à exécuter l'application dans une image du navigateur (ms), pour ne pas figer la page. */
const MAX_SPEED_WALL_MS = 12;

/** Instructions au plus par image du navigateur (au-delà, on rend la main pour ne pas figer la page). */
const DEFAULT_SLICE = 2_000_000;

export class Player {
  readonly nwasabi: Nwasabi;
  readonly state: PlayerState;
  private readonly slice: number;
  private readonly maxWallMs: number;
  private readonly maxElapsedMs: number;
  private readonly scheduler: Scheduler;
  private readonly listeners = new Set<(frame: Frame) => void>();
  private canvas: CanvasLike | null = null;
  private image: RgbaTarget | null = null;
  private last = 0;
  private frame: unknown = null;
  private nwa: Uint8Array | null = null;
  private data: Uint8Array | null = null;
  private readonly pressed = new Set<KeyName>();
  // Une touche relâchée avant que l'application ait pu la lire (un appui plus court qu'une image)
  // reste enfoncée jusqu'à l'image suivante.
  private readonly sampled = new Set<KeyName>();
  private readonly deferred = new Set<KeyName>();
  /** Clavier de l'ordinateur et gestes tactiles → touches de la calculatrice (voir `KeyMap`). */
  readonly input: InputMapper;

  /** L'état de départ d'un lecteur (à rendre réactif avant de le donner à `create`, si besoin). */
  static initialState(): PlayerState {
    return {
      loaded: false,
      running: false,
      model: "n0110",
      firmware: "24",
      speed: "model",
      customMhz: DEFAULT_CUSTOM_MHZ,
      limitMhz: null,
      appName: "",
      stop: null,
      frames: 0,
      instructions: 0,
      error: "",
      stdout: "",
      timeMs: 0,
      keymapName: null,
      needsData: false,
      revision: 0,
      capHits: 0,
    };
  }

  private constructor(nwasabi: Nwasabi, options: PlayerOptions) {
    this.nwasabi = nwasabi;
    this.state = options.state ?? Player.initialState();
    this.scheduler = options.scheduler ?? defaultScheduler();
    if (options.onFrame) this.listeners.add(options.onFrame);
    if (options.speed) this.state.speed = options.speed;
    if (options.customMhz !== undefined) this.state.customMhz = options.customMhz;
    this.input = new InputMapper(this, options.keymap ?? KeyMap.default());
    this.state.keymapName = this.input.keymap.name;
    this.slice = options.slice ?? DEFAULT_SLICE;
    this.maxWallMs = options.maxWallMs ?? MAX_SPEED_WALL_MS;
    this.maxElapsedMs = options.maxElapsedMs ?? DEFAULT_MAX_ELAPSED_MS;
  }

  /** Charge le module WebAssembly (à fournir, voir `Nwasabi.load`) et crée un appareil vide. */
  static async create(wasm: WasmSource, options: PlayerOptions = {}): Promise<Player> {
    const player = new Player(await Nwasabi.load(wasm), options);
    player.newDevice();
    return player;
  }

  /** Les champs de vitesse à joindre aux requêtes qui créent l'appareil. */
  private speedFields(): { speed: SpeedMode; mhz: number } {
    return { speed: this.state.speed, mhz: this.state.customMhz };
  }

  /** Retient la fréquence appliquée que le module annonce dans sa réponse. */
  private readSpeed(reply: Reply): void {
    if (reply.ok && "limit_mhz" in reply) this.state.limitMhz = (reply.limit_mhz as number | null) ?? null;
  }

  /** Crée un appareil vide du modèle et de la vitesse choisis. */
  private newDevice(): Reply {
    const reply = this.nwasabi.request({
      cmd: "new",
      model: this.state.model,
      firmware: this.state.firmware,
      clock: "host",
      ...this.speedFields(),
    });
    this.readSpeed(reply);
    return reply;
  }

  /** Choisit la vitesse du processeur ; elle s'applique tout de suite, sans relancer l'application. */
  setSpeed(speed: SpeedMode): void {
    const previous = this.state.speed;
    this.state.speed = speed;
    if (!this.pushSpeed()) this.state.speed = previous;
  }

  /** Change la fréquence du mode `"custom"` (en MHz) ; sans effet tant qu'un autre mode est choisi. Une valeur refusée par le module (hors de 1 à 100 000) est ignorée. */
  setCustomMhz(mhz: number): void {
    const previous = this.state.customMhz;
    this.state.customMhz = mhz;
    if (!this.pushSpeed()) this.state.customMhz = previous;
  }

  private pushSpeed(): boolean {
    const reply = this.nwasabi.request({ cmd: "speed", ...this.speedFields() });
    if (reply.ok) this.readSpeed(reply);
    else this.state.error = reply.error ?? "vitesse refusée";
    return reply.ok;
  }

  /** Envoie une requête au module ; les erreurs sont gardées dans `state.error`. */
  call(request: Request): Reply {
    const reply = this.nwasabi.request(request);
    if (!reply.ok) this.state.error = reply.error ?? "erreur inconnue";
    return reply;
  }

  /** Dessine l'écran dans ce canvas (320×240) à chaque image. Facultatif : les pixels bruts sont disponibles sans canvas. */
  attach(canvas: CanvasLike): void {
    this.canvas = canvas;
    this.image = canvas.getContext("2d")?.createImageData(SCREEN_WIDTH, SCREEN_HEIGHT) ?? null;
    this.draw();
  }

  /** Recopie l'écran de l'appareil dans le canvas attaché, s'il y en a un. */
  draw(): void {
    const context = this.canvas?.getContext("2d");
    if (context && this.image && this.nwasabi.drawTo(this.image)) {
      context.putImageData(this.image as never, 0, 0);
    }
  }

  /** Pixels de l'écran en RGB565 (320×240), valables jusqu'à la prochaine requête au module ; `null` sans appareil. */
  framebuffer(): Uint16Array | null {
    return this.nwasabi.framebuffer();
  }

  /** Copie de l'écran en RGBA 8 bits (320×240×4 octets) ; `null` sans appareil. */
  rgba(): Uint8ClampedArray | null {
    return this.nwasabi.rgba();
  }

  /**
   * Reçoit chaque image de l'écran, brute, au rythme de la simulation (à la fin de chaque pas de la boucle).
   * Rend la fonction qui retire l'écouteur.
   */
  onFrame(listener: (frame: Frame) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private emitFrame(): void {
    if (this.listeners.size === 0) return;
    const rgb565 = this.nwasabi.framebuffer();
    if (!rgb565) return;
    const frame: Frame = {
      width: SCREEN_WIDTH,
      height: SCREEN_HEIGHT,
      rgb565,
      rgba: () => this.nwasabi.rgba() ?? new Uint8ClampedArray(0),
      timeMs: this.state.timeMs,
    };
    for (const listener of this.listeners) listener(frame);
  }

  /**
   * Temps de simulation écoulé depuis le début de l'application, en millisecondes, à l'instant de
   * l'appel. C'est le temps que voit l'application elle-même (`timing_millis`). Il continue de
   * défiler pendant qu'elle dort et reste figé tant que le lecteur est en pause.
   */
  time(): number {
    const reply = this.nwasabi.request({ cmd: "time" });
    return reply.ok ? (reply.time_us as number) / 1000 : this.state.timeMs;
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
    state.timeMs = 0;
    this.pressed.clear();
    this.sampled.clear();
    this.deferred.clear();
    const request: Request = {
      cmd: "load",
      model: state.model,
      firmware: state.firmware,
      clock: "host", // le temps de l'application est celui que ce lecteur lui donne, jamais plus
      ...this.speedFields(),
      nwa: toBase64(this.nwa),
    };
    if (this.data) request.data = toBase64(this.data);
    state.needsData = false;
    const reply = this.call(request);
    if (!reply.ok && !this.data && /données externes|external/i.test(reply.error ?? "")) {
      // L'application réclame des données externes : on les attend (voir `runWithoutData` pour s'en passer).
      state.error = "";
      state.needsData = true;
      state.loaded = false;
      this.draw();
      this.bump();
      return false;
    }
    state.loaded = reply.ok;
    this.readSpeed(reply);
    this.draw();
    this.bump();
    if (reply.ok) this.start();
    return reply.ok;
  }

  /** Les parties que l'on peut enregistrer d'un état (voir `dump`). */
  static readonly DUMP_PARTS = ["app", "memory", "fs", "state"] as const;

  /**
   * Enregistre l'état de l'appareil (document `.nds`, voir `docs/format/nds.md`). L'émulation doit être en pause :
   * l'état est pris entre deux instructions. Rend `null` (et `state.error`) sinon, ou si le module refuse.
   */
  dump(parts: readonly string[] = Player.DUMP_PARTS): unknown | null {
    if (!this.state.loaded) return null;
    if (this.state.running) {
      this.state.error = "pause requise pour enregistrer l'état";
      return null;
    }
    const reply = this.call({ cmd: "dump", parts: [...parts] });
    if (!reply.ok) return null;
    // Le nom du fichier est celui que connaît l'hôte, pas le cœur : on l'ajoute (champ informatif).
    const document = reply.nds as { app?: { file_name?: string } };
    if (document.app && this.state.appName) document.app.file_name = this.state.appName;
    return document;
  }

  /**
   * Remplace l'appareil par l'état d'un document `.nds`. L'émulation reste **en pause** : c'est à l'appelant de la
   * reprendre (`start()`). Rend `false` (et `state.error`) si le document est refusé : l'appareil n'est alors pas touché.
   */
  restore(document: unknown): boolean {
    this.stop();
    const reply = this.call({ cmd: "restore", nds: document });
    if (!reply.ok) return false;
    const state = this.state;
    const doc = document as {
      app?: { nwa?: string; data?: string; name?: string; file_name?: string };
    };
    if (doc.app?.nwa) {
      this.nwa = fromBase64(doc.app.nwa);
      this.data = doc.app.data !== undefined ? fromBase64(doc.app.data) : null;
    }
    const app = reply.app as { name?: string } | undefined;
    if (doc.app?.file_name || app?.name) state.appName = doc.app?.file_name ?? app?.name ?? state.appName;
    state.model = String(reply.model).toLowerCase() as Model;
    state.firmware = reply.firmware as Firmware;
    if (reply.speed) state.speed = reply.speed as SpeedMode;
    if (typeof reply.mhz === "number") state.customMhz = reply.mhz;
    this.readSpeed(reply);
    state.error = "";
    state.loaded = true;
    state.stop = null;
    state.frames = (reply.frames as number | undefined) ?? 0;
    const info = this.call({ cmd: "time" });
    state.timeMs = info.ok ? (info.time_us as number) / 1000 : 0;
    const out = this.call({ cmd: "stdout" });
    state.stdout = (out.text as string | undefined) ?? "";
    this.pressed.clear();
    this.sampled.clear();
    this.deferred.clear();
    this.input.releaseAll();
    this.draw();
    this.emitFrame();
    this.bump();
    return true;
  }

  /** Lance l'application qui réclame des données externes avec un jeu vide. */
  runWithoutData(): boolean {
    if (!this.nwa) return false;
    return this.load(this.nwa, this.state.appName, new Uint8Array());
  }

  /** Change le modèle (et le firmware) ; relance l'application chargée, s'il y en a une. */
  changeModel(model: Model, firmware: Firmware = this.state.firmware): void {
    this.state.model = model;
    this.state.firmware = firmware;
    if (this.nwa) {
      this.restart();
    } else {
      this.newDevice();
      this.bump();
    }
  }

  start(): void {
    if (!this.state.loaded || this.state.running) return;
    this.state.running = true;
    this.last = this.scheduler.now();
    this.frame = this.scheduler.request((now) => this.tick(now));
  }

  stop(): void {
    this.state.running = false;
    if (this.frame !== null) this.scheduler.cancel(this.frame);
    this.frame = null;
  }

  private advance(ms: number): void {
    if (ms > 0) this.call({ cmd: "advance", us: Math.round(ms * 1000) });
  }

  /**
   * Une image du navigateur. L'application ne voit passer que le temps réellement écoulé depuis la
   * dernière image, jamais plus : ce temps est un « budget » qu'elle consomme en dormant ou en
   * attendant une touche, et dont le reste lui est donné à la fin. En pause (`stop()`), aucun temps ne
   * lui est donné : pour elle, le temps s'arrête.
   */
  private tick(now: number): void {
    const state = this.state;
    if (!state.running) return;
    // Un écran plus rapide que 60 Hz : on saute des images pour garder la cadence de la calculatrice.
    if (now - this.last < FRAME_MS - 1) {
      this.frame = this.scheduler.request((time) => this.tick(time));
      return;
    }
    let budget = Math.max(0, Math.min(now - this.last, this.maxElapsedMs));
    this.last = now;
    const mhz = state.limitMhz;
    const wallStart = this.scheduler.now();
    for (let runs = 0; runs < MAX_RUNS_PER_TICK && state.running; runs++) {
      // Avec une fréquence, l'application ne peut exécuter que ce que le processeur fait pendant le temps
      // qui lui reste (MHz × microsecondes) ; sans limite, ce qu'une tranche permet.
      const room = mhz === null ? Infinity : Math.floor(mhz * budget * 1000);
      if (room < 1) {
        this.advance(budget);
        break;
      }
      const reply = this.call({ cmd: "run", instructions: Math.min(this.slice, room), frames: 1 });
      if (!reply.ok) return this.stop();
      for (const name of this.pressed) this.sampled.add(name);
      for (const name of [...this.deferred]) {
        this.deferred.delete(name);
        this.release(name);
      }
      const stop = reply.stop as StopInfo;
      state.stop = stop;
      const executed = reply.instructions as number;
      state.instructions += executed;
      state.frames += reply.frames as number;
      // Le temps que le processeur a mis à exécuter ces instructions passe pour l'application.
      if (mhz !== null && executed > 0) {
        const spent = Math.min(budget, executed / (mhz * 1000));
        this.advance(spent);
        budget -= spent;
      }
      if (stop.kind === "sleep") {
        // L'application dort : le temps passe pour elle, dans la limite du budget.
        const wanted = (stop.ms as number) ?? 0;
        const step = Math.min(wanted, budget);
        this.advance(step);
        budget -= step;
        if (budget > 0) continue;
        break;
      }
      // Une application occupée sans fin d'image continue tant qu'il lui reste du temps de processeur (ou
      // sans limite de vitesse) et qu'on ne fige pas la page.
      if (stop.kind === "budget" && (mhz === null || budget > 0) && this.scheduler.now() - wallStart < this.maxWallMs) {
        continue;
      }
      if (stop.kind === "budget" && mhz !== null && budget > 0) state.capHits++;
      this.advance(budget);
      budget = 0;
      if (!["frame", "wait_event", "budget"].includes(stop.kind)) {
        state.running = false; // fin, faute, point d'observation, Home…
      }
      break;
    }
    state.timeMs = this.time();
    this.draw();
    this.emitFrame();
    if (state.frames % 10 === 0 || !state.running) {
      const out = this.call({ cmd: "stdout" });
      state.stdout = (out.text as string | undefined) ?? state.stdout;
    }
    if (!state.running) this.bump();
    else this.frame = this.scheduler.request((time) => this.tick(time));
  }

  private release(name: KeyName): void {
    this.pressed.delete(name);
    this.sampled.delete(name);
    this.call({ cmd: "key", key: name, action: "release" });
  }

  /** Remplace le mapping des touches (clavier de l'ordinateur et gestes) ; ce qui était tenu est relâché. */
  setKeyMap(map: KeyMap): void {
    this.input.setKeyMap(map);
    this.state.keymapName = map.name;
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
