// Instantanés de la mémoire : on photographie les zones modifiables (RAM), puis on compare avec l'état du moment pour voir
// ce qui a changé, par plage d'octets.

export interface Snapshot {
  takenAt: number;
  /** Une copie par zone : adresse de départ et octets. */
  parts: Array<{ base: number; bytes: Uint8Array }>;
}

export interface Change {
  address: number;
  length: number;
  before: Uint8Array;
  after: Uint8Array;
}

/** Les plages d'octets qui diffèrent entre deux copies de la même zone. Des différences séparées de moins de `gap` octets forment une plage. */
export function diffBytes(base: number, before: Uint8Array, after: Uint8Array, gap = 4): Change[] {
  const changes: Change[] = [];
  const length = Math.min(before.length, after.length);
  let start = -1;
  let last = -1;
  const close = () => {
    if (start < 0) return;
    changes.push({ address: base + start, length: last - start + 1, before: before.slice(start, last + 1), after: after.slice(start, last + 1) });
    start = -1;
  };
  for (let i = 0; i < length; i++) {
    if (before[i] === after[i]) continue;
    if (start >= 0 && i - last > gap) close();
    if (start < 0) start = i;
    last = i;
  }
  close();
  return changes;
}

/** Toutes les différences entre un instantané et l'état actuel (mêmes zones, dans le même ordre). */
export function compare(snapshot: Snapshot, now: Array<{ base: number; bytes: Uint8Array }>): Change[] {
  return snapshot.parts.flatMap((part, i) => (now[i] ? diffBytes(part.base, part.bytes, now[i].bytes) : []));
}
