// Dossier de .nwa de tiers pour les tests : argument, sinon NWASABI_CORPUS, sinon `local/nwa` s'il existe.
// Sans aucun des trois, les tests qui en ont besoin sont ignorés avec un message ; avec NWASABI_REQUIRE_CORPUS=1, ils échouent.
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export function corpusDir() {
  const given = process.argv[2] || process.env.NWASABI_CORPUS;
  if (given) return given;
  const fallback = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "local", "nwa");
  if (existsSync(fallback)) return fallback;
  const required = process.env.NWASABI_REQUIRE_CORPUS;
  if (required && required !== "0") {
    console.error("NWASABI_REQUIRE_CORPUS est demandé mais aucun corpus n'est disponible (argument, NWASABI_CORPUS ou local/nwa)");
    process.exit(1);
  }
  console.error("test ignoré : pas de corpus d'applications (argument, NWASABI_CORPUS ou local/nwa)");
  return undefined;
}
