// Le débogage du code source : ce que dit le cœur des informations DWARF de l'application (fichiers, lignes, pile, variables),
// et le texte des fichiers source, que le navigateur ne peut pas lire tout seul : l'utilisateur les donne (choix ou glisser-déposer),
// et ils sont gardés dans le stockage du site, par application.

import { reactive } from "vue";
import type { Emulator } from "./emulator.js";
import { idbGet, idbSet } from "./idb.js";

export interface SourceFile {
  id: number;
  path: string;
}

export interface FrameInfo {
  index: number;
  pc: number;
  function?: string;
  symbol?: string;
  file?: number;
  path?: string;
  line?: number;
  estimated?: boolean;
}

/** Une valeur lue par le cœur : son résumé, et ses champs ou éléments. */
export interface VarValue {
  name?: string;
  type: string;
  value: string;
  address?: number;
  size: number;
  children?: VarValue[];
  count?: number;
  param?: boolean;
  pointer?: number;
}

export const source = reactive({
  /** L'application a des informations de débogage du code source. */
  available: false,
  files: [] as SourceFile[],
  /** Texte des fichiers déjà donnés, par chemin DWARF. */
  texts: {} as Record<string, string>,
  /** Le fichier affiché. */
  current: null as string | null,
  /** Le cadre de la pile choisi (0 : l'instruction en cours). */
  frame: 0,
  /** Les lignes qui ont du code, par chemin. */
  codeLines: {} as Record<string, number[]>,
  /** Expressions suivies en permanence. */
  watches: [] as string[],
  app: "",
});

const key = (app: string, path: string) => `nwasabi.source.${app}.${path}`;

/** Relit les informations de débogage de l'application chargée. */
export async function refreshDebugInfo(emulator: Emulator, app: string): Promise<void> {
  const info = await emulator.quiet({ cmd: "debug_info" });
  source.app = app;
  source.available = info.ok && info.available === true;
  source.files = source.available ? ((info.files as SourceFile[]) ?? []) : [];
  source.texts = {};
  source.codeLines = {};
  source.frame = 0;
  source.current = source.files[0]?.path ?? null;
  for (const file of source.files) {
    const stored = await idbGet<string>(key(app, file.path));
    if (typeof stored === "string") source.texts[file.path] = stored;
    const lines = await emulator.quiet({ cmd: "source_lines", file_id: file.id });
    if (lines.ok) source.codeLines[file.path] = lines.lines as number[];
  }
  // Le fichier qui définit `main`, s'il est connu, est un meilleur départ que le premier venu.
  const main = await emulator.quiet({ cmd: "break_function", name: "main" });
  if (main.ok && typeof main.path === "string") {
    await emulator.quiet({ cmd: "unbreak", address: main.address });
    if (source.files.some((f) => f.path === main.path)) source.current = main.path as string;
  }
}

const basename = (path: string) => path.replace(/\\/g, "/").split("/").pop() ?? path;

/** Associe des fichiers donnés par l'utilisateur aux chemins du DWARF (par le nom du fichier, ou le chemin relatif s'il y en a un). */
export async function addSourceFiles(files: File[]): Promise<number> {
  let matched = 0;
  for (const file of files) {
    const text = await file.text();
    const relative = (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name;
    const candidates = source.files.filter((f) => basename(f.path) === file.name);
    if (!candidates.length) continue;
    // Plusieurs fichiers de même nom : celui dont le chemin se termine comme le chemin relatif.
    const best =
      candidates.find((c) => c.path.replace(/\\/g, "/").endsWith(relative.replace(/\\/g, "/"))) ??
      candidates.find((c) => source.texts[c.path] === undefined) ??
      candidates[0];
    if (!best) continue;
    source.texts[best.path] = text;
    await idbSet(key(source.app, best.path), text);
    matched++;
  }
  return matched;
}

export const sourceLines = (path: string | null): string[] | null => {
  if (!path) return null;
  const text = source.texts[path];
  return text === undefined ? null : text.replace(/\r\n?/g, "\n").split("\n");
};

/** Les valeurs visibles à un cadre : paramètres, variables locales. */
export async function readLocals(emulator: Emulator, frame: number): Promise<{ params: VarValue[]; locals: VarValue[] }> {
  const reply = await emulator.quiet({ cmd: "locals", frame });
  if (!reply.ok) return { params: [], locals: [] };
  return { params: (reply.params as VarValue[]) ?? [], locals: (reply.locals as VarValue[]) ?? [] };
}

export async function evaluateExpression(emulator: Emulator, expr: string, frame: number): Promise<VarValue & { error?: string }> {
  const reply = await emulator.quiet({ cmd: "eval", expr, frame });
  if (!reply.ok) return { type: "", value: "", size: 0, error: String(reply.error) };
  return reply as unknown as VarValue;
}
