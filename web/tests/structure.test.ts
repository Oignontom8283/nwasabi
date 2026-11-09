// La structure HTML (balises, classes, attributs, textes) des composants : une référence enregistrée garde l'interface identique
// pendant les remaniements. Ce n'est pas une comparaison de pixels : le CSS, lui, n'est pas touché par ces tests.
import { mount } from "@vue/test-utils";
import { reactive } from "vue";
import { describe, expect, it } from "vitest";
import { KeyMap } from "@nwasabi/js";
import { i18n } from "../src/i18n.js";
import CalculatorSvg from "../src/components/CalculatorSvg.vue";

function fakeEmulator() {
  const state = reactive({ keymapVersion: 0, loaded: false, running: false, needsData: false, appName: "" });
  return { state, keymap: () => KeyMap.default(), press: () => {}, attach: () => {} };
}

const global = () => ({ plugins: [i18n], provide: { emulator: fakeEmulator() }, stubs: { ScreenView: true } });

describe("structure des composants", () => {
  it("la calculatrice (mode jeu)", () => {
    expect(mount(CalculatorSvg, { global: global() }).html()).toMatchSnapshot();
  });
  it("la calculatrice (vue du mapping)", () => {
    expect(mount(CalculatorSvg, { props: { mode: "map" }, global: global() }).html()).toMatchSnapshot();
  });
});
