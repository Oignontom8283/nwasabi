// Les raccourcis clavier : chaque combinaison donne la commande attendue, les modificateurs sont exacts et la position de la touche prime
// sur le caractère (un clavier AZERTY donne les mêmes raccourcis qu'un QWERTY).
import { describe, expect, it } from "vitest";
import { shortcutOf } from "../src/shortcuts.js";

const press = (init: KeyboardEventInit) => shortcutOf(new KeyboardEvent("keydown", init));

describe("raccourcis", () => {
  it("Ctrl ou Cmd + touche", () => {
    expect(press({ code: "Space", key: " ", ctrlKey: true })).toBe("toggleRun");
    expect(press({ code: "Space", key: " ", metaKey: true })).toBe("toggleRun");
    expect(press({ code: "Space", key: " ", ctrlKey: true, shiftKey: true })).toBe("restart");
    expect(press({ code: "Period", key: ".", ctrlKey: true })).toBe("frame");
    expect(press({ code: "KeyE", key: "e", ctrlKey: true })).toBe("capture");
    expect(press({ code: "KeyE", key: "E", ctrlKey: true, shiftKey: true })).toBe("copy");
    expect(press({ code: "KeyG", key: "g", ctrlKey: true })).toBe("gif");
    expect(press({ code: "KeyG", key: "G", ctrlKey: true, shiftKey: true })).toBe("video");
    expect(press({ code: "Enter", key: "Enter", ctrlKey: true })).toBe("fullscreen");
    expect(press({ code: "KeyB", key: "b", ctrlKey: true })).toBe("large");
    expect(press({ code: "KeyJ", key: "j", ctrlKey: true })).toBe("panel");
    expect(press({ code: "KeyS", key: "s", ctrlKey: true })).toBe("quickSave");
    expect(press({ code: "KeyO", key: "o", ctrlKey: true })).toBe("quickLoad");
    expect(press({ code: "KeyR", key: "r", ctrlKey: true })).toBe("reload");
  });
  it("les anciennes combinaisons avec Alt", () => {
    expect(press({ code: "KeyP", key: "p", altKey: true })).toBe("toggleRun");
    expect(press({ code: "KeyR", key: "r", altKey: true, shiftKey: true })).toBe("restart");
    expect(press({ code: "KeyG", key: "G", altKey: true, shiftKey: true })).toBe("video");
    expect(press({ code: "KeyT", key: "t", altKey: true })).toBe("panel");
  });
  it("les modificateurs sont exacts", () => {
    expect(press({ code: "KeyS", key: "S", ctrlKey: true, shiftKey: true })).toBeNull();
    expect(press({ code: "KeyS", key: "s", ctrlKey: true, altKey: true })).toBeNull();
    expect(press({ code: "KeyS", key: "s" })).toBeNull();
    expect(press({ code: "KeyZ", key: "z", ctrlKey: true })).toBeNull();
  });
  it("la position de la touche prime sur le caractère (AZERTY)", () => {
    expect(press({ code: "KeyE", key: "é", ctrlKey: true })).toBe("capture");
  });
});
