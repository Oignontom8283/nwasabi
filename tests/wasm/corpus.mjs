// Dossier de .nwa de tiers pour les tests : argument, sinon NWASABI_CORPUS, sinon `local/nwa` s'il existe.
// Sans aucun des trois, les tests qui en ont besoin sont ignorés en silence.
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export function corpusDir() {
  const given = process.argv[2] || process.env.NWASABI_CORPUS;
  if (given) return given;
  const fallback = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "local", "nwa");
  return existsSync(fallback) ? fallback : undefined;
}
