import { describe, expect, it } from "vitest";
import { searchCommands } from "../src/paletteSearch.js";

const commands = ["Load an application…", "Pause", "Resume", "Restart the application", "Record a GIF", "Open the panel", "Charger une applicâtion"].map((label) => ({ label }));
const labels = (q: string) => searchCommands(commands, q).map((c) => c.label);

describe("recherche de la palette", () => {
  it("un champ vide garde tout, dans l'ordre", () => expect(labels("")).toEqual(commands.map((c) => c.label)));
  it("trouve par des lettres du nom, sans casse ni accent", () => {
    expect(labels("GIF")).toEqual(["Record a GIF"]);
    expect(labels("applicat")).toContain("Charger une applicâtion");
  });
  it("classe la meilleure correspondance en premier", () => expect(labels("pan")[0]).toBe("Open the panel"));
  it("ne garde rien quand rien ne correspond", () => expect(labels("zzz")).toEqual([]));
});
