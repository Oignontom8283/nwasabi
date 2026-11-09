// La recherche de la palette de commandes : tolérante (lettres éparpillées acceptées si elles se suivent de près) et classée, la
// meilleure correspondance d'abord. Sans accents ni casse. Un champ vide garde toutes les commandes dans l'ordre de la liste.
import fuzzysort from "fuzzysort";

const plain = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

export function searchCommands<T extends { label: string }>(commands: readonly T[], query: string): T[] {
  const q = plain(query).replace(/\s+/g, "");
  if (!q) return [...commands];
  const items = commands.map((command) => ({ text: plain(command.label), command }));
  return fuzzysort.go(q, items, { key: "text", limit: 0 }).map((r) => r.obj.command);
}
