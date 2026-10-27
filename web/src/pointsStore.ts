// Les points d'arrêt et d'observation vus ensemble. Le cœur ne connaît que les points actifs ; ceux qu'on désactive sont retirés
// du cœur et gardés ici, pour être reposés à la réactivation.

import { reactive } from "vue";
import type { Emulator } from "./emulator.js";

export interface Condition {
  register: string;
  op: string;
  value: number;
}
export interface BreakPoint {
  address: number;
  condition?: Condition;
}
export interface WatchPoint {
  id: number;
  address: number;
  length: number;
  kind: string;
  stop: boolean;
}
export type DisabledPoint = { kind: "break"; point: BreakPoint } | { kind: "watch"; point: Omit<WatchPoint, "id"> };

export const points = reactive({
  breaks: [] as BreakPoint[],
  watches: [] as WatchPoint[],
  disabled: [] as DisabledPoint[],
});

export async function refreshPoints(emulator: Emulator): Promise<void> {
  const b = await emulator.quiet({ cmd: "breaks" });
  const w = await emulator.quiet({ cmd: "watch_list" });
  const breaks = b.ok ? (b.details as BreakPoint[]) : [];
  const watches = w.ok ? (w.watches as WatchPoint[]) : [];
  if (JSON.stringify(breaks) !== JSON.stringify(points.breaks)) points.breaks = breaks;
  if (JSON.stringify(watches) !== JSON.stringify(points.watches)) points.watches = watches;
}

/** Pose un point d'arrêt (avec sa condition éventuelle) ; rend le message d'erreur du cœur, ou `null`. */
export async function addBreak(emulator: Emulator, address: number, condition?: Condition): Promise<string | null> {
  const reply = await emulator.quiet({ cmd: "break", address, condition: condition ?? null });
  if (!reply.ok) return String(reply.error);
  await refreshPoints(emulator);
  return null;
}

export async function removeBreak(emulator: Emulator, address: number): Promise<void> {
  await emulator.quiet({ cmd: "unbreak", address });
  await refreshPoints(emulator);
}

export async function removeWatchPoint(emulator: Emulator, id: number): Promise<void> {
  await emulator.quiet({ cmd: "unwatch", id });
  await refreshPoints(emulator);
}

/** Désactive un point actif : il quitte le cœur mais reste dans la liste. */
export async function disable(emulator: Emulator, entry: DisabledPoint): Promise<void> {
  if (entry.kind === "break") await emulator.quiet({ cmd: "unbreak", address: entry.point.address });
  else {
    const found = points.watches.find((w) => w.address === entry.point.address && w.length === entry.point.length && w.kind === entry.point.kind);
    if (found) await emulator.quiet({ cmd: "unwatch", id: found.id });
  }
  points.disabled.push(entry);
  await refreshPoints(emulator);
}

/** Réactive un point désactivé (le repose dans le cœur). */
export async function enable(emulator: Emulator, index: number): Promise<void> {
  const [entry] = points.disabled.splice(index, 1);
  if (!entry) return;
  if (entry.kind === "break") await addBreak(emulator, entry.point.address, entry.point.condition);
  else await emulator.quiet({ cmd: "watch", address: entry.point.address, length: entry.point.length, kind: entry.point.kind, stop: entry.point.stop });
  await refreshPoints(emulator);
}

/** Vide la liste des points désactivés (nouvelle application, par exemple). */
export const forgetDisabled = (): void => void (points.disabled = []);

/** `reg op valeur` : la condition d'un `break … if …`, ou une erreur. */
export function parseCondition(text: string): Condition | null {
  const m = /^\s*\$?(r\d{1,2}|sp|lr|pc)\s*(==|!=|<=|>=|<|>|&)\s*(0x[0-9a-f]+|\d+)\s*$/i.exec(text);
  return m ? { register: (m[1] as string).toLowerCase(), op: m[2] as string, value: Number(m[3]) } : null;
}

export const describeCondition = (c: Condition): string => `${c.register} ${c.op} ${c.value}`;
