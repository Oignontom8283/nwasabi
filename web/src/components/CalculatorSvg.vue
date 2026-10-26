<script setup lang="ts">
import { useEmu } from "../emulator.js";
import { computed, inject } from "vue";
import { useI18n } from "vue-i18n";
import type { KeyName, TouchRule } from "@nwasabi/js";
import { BEZEL, BODY, CROSS, KEYS, SCREEN, VIEW } from "../calc-layout.js";
import type { CalcKey } from "../calc-layout.js";
import { ui } from "../ui.js";
import ScreenView from "./ScreenView.vue";

const props = withDefaults(
  defineProps<{
    /** "play" : la calculatrice qui tourne ; "map" : vue du mapping (touches sélectionnables, écran éteint). */
    mode?: "play" | "map";
    /** Touche sélectionnée (mode "map"). */
    selected?: string;
  }>(),
  { mode: "play", selected: "" },
);
const emit = defineEmits<{ select: [key: KeyName] }>();
type Sources = { keyboard: string[]; rules: TouchRule[] };

const { t } = useI18n();
const emulator = useEmu();
const { state } = emulator;

const keymap = computed(() => (state.keymapVersion >= 0 ? emulator.keymap() : null));
const sources = computed(() => {
  const out: Partial<Record<KeyName, Sources>> = {};
  const map = keymap.value;
  if (!map) return out;
  for (const key of KEYS) out[key.name] = map.sourcesOf(key.name);
  return out;
});

const pct = (value: number, total: number) => `${(value / total) * 100}%`;
const screenStyle = {
  left: pct(SCREEN.x, VIEW.width),
  top: pct(SCREEN.y, VIEW.height),
  width: pct(SCREEN.width, VIEW.width),
  height: pct(SCREEN.height, VIEW.height),
};

const GESTURE_KEYS: Record<string, string> = {
  tap: "gesture.tap", double_tap: "gesture.double_tap", long_press: "gesture.long_press",
  swipe_left: "gesture.swipe_left", swipe_right: "gesture.swipe_right", swipe_up: "gesture.swipe_up", swipe_down: "gesture.swipe_down",
};
const hostLabel = (id: string) => (id.startsWith("key:") ? `« ${id.slice(4)} »` : id);

// Les touches liées ne sont colorées que dans la vue du mapping, pas sur la calculatrice qui tourne.
function mapped(name: KeyName) {
  if (props.mode !== "map") return false;
  const s = sources.value[name];
  return !!s && (s.keyboard.length > 0 || s.rules.length > 0);
}

function tooltip(key: CalcKey) {
  const s = sources.value[key.name];
  const lines = [`${key.label} (${key.name})`];
  if (s?.keyboard.length) lines.push(`${t("calc.keyboard")} : ${s.keyboard.map(hostLabel).join(", ")}`);
  for (const rule of s?.rules ?? []) {
    const fingers = rule.fingers > 1 ? ` ×${rule.fingers}` : "";
    lines.push(`${t(GESTURE_KEYS[rule.gesture] ?? rule.gesture)}${fingers}${rule.whileHeld ? ` (${t("calc.held")})` : ""}`);
  }
  if (lines.length === 1) lines.push(t("calc.notMapped"));
  return lines.join("\n");
}

// Taille du texte d'une touche : plus il est court, plus il est grand ; une légende au-dessus limite la taille.
function labelSize(key: CalcKey) {
  const n = [...key.label].length;
  let size = key.style === "small" ? 28 : n <= 1 ? 54 : n === 2 ? 46 : n === 3 ? 40 : n <= 5 ? 34 : 30;
  if (key.shift || key.alpha) size = Math.min(size, key.height < 95 ? 40 : 50);
  return size;
}
const labelY = (key: CalcKey) => (key.shift || key.alpha ? key.y + key.height * 0.8 : key.y + key.height / 2 + labelSize(key) * 0.35);

// Un appui (souris, doigt) reste enfoncé tant que le bouton est maintenu.
const held = new Map<number, KeyName>();
function down(key: CalcKey, event: PointerEvent) {
  if (props.mode === "map") return emit("select", key.name);
  (event.currentTarget as Element).setPointerCapture?.(event.pointerId);
  held.set(event.pointerId, key.name);
  emulator.press(key.name, true);
}
function up(event: PointerEvent) {
  const name = held.get(event.pointerId);
  if (!name) return;
  held.delete(event.pointerId);
  emulator.press(name, false);
}
function keydown(key: CalcKey, event: KeyboardEvent) {
  if (event.repeat || (event.key !== "Enter" && event.key !== " ")) return;
  event.preventDefault();
  event.stopPropagation();
  if (props.mode === "map") emit("select", key.name);
  else emulator.press(key.name, true);
}
function keyup(key: CalcKey, event: KeyboardEvent) {
  if (props.mode === "map" || (event.key !== "Enter" && event.key !== " ")) return;
  event.preventDefault();
  event.stopPropagation();
  emulator.press(key.name, false);
}
</script>

<template>
  <div class="calc" :class="[mode, { large: mode === 'play' && ui.large }]" :style="{ '--ratio': VIEW.width / VIEW.height }">
    <svg v-show="!(mode === 'play' && ui.large)" :viewBox="`0 0 ${VIEW.width} ${VIEW.height}`" xmlns="http://www.w3.org/2000/svg" role="group" :aria-label="t('calc.label')">
      <defs>
        <linearGradient id="key-grad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#ffffff" />
          <stop offset="1" stop-color="#ebebeb" />
        </linearGradient>
      </defs>
      <rect :x="BODY.x" :y="BODY.y" :width="BODY.width" :height="BODY.height" :rx="BODY.radius" class="body" />
      <text :x="BODY.x + BODY.width / 2" y="84" text-anchor="middle" class="brand">nwasabi</text>
      <rect :x="BEZEL.x" :y="BEZEL.y" :width="BEZEL.width" :height="BEZEL.height" :rx="BEZEL.radius" class="bezel" />
      <rect :x="SCREEN.x - 10" :y="SCREEN.y - 10" :width="SCREEN.width + 20" :height="SCREEN.height + 20" rx="10" fill="#111" />
      <rect v-if="mode === 'map'" :x="SCREEN.x" :y="SCREEN.y" :width="SCREEN.width" :height="SCREEN.height" rx="4" fill="#1d1d1f" />
      <text v-if="mode === 'map'" :x="SCREEN.x + SCREEN.width / 2" :y="SCREEN.y + SCREEN.height / 2" text-anchor="middle" fill="#9a9aa0" font-size="44" font-family="system-ui, sans-serif">{{ t("calc.mapHint") }}</text>
      <g class="cross">
        <rect v-for="(arm, i) in CROSS" :key="'l' + i" class="cross-lip" :x="arm.x" :y="arm.y + 7" :width="arm.width" :height="arm.height" :rx="arm.radius" />
        <rect v-for="(arm, i) in CROSS" :key="i" :x="arm.x" :y="arm.y" :width="arm.width" :height="arm.height" :rx="arm.radius" />
      </g>

      <g
        v-for="key in KEYS"
        :key="key.name"
        class="key"
        :class="[key.style, { mapped: mapped(key.name), selected: selected === key.name }]"
        :data-calc-key="key.name"
        role="button"
        tabindex="0"
        :aria-label="tooltip(key).replace(/\n/g, '. ')"
        @pointerdown.prevent="down(key, $event)"
        @pointerup="up"
        @pointercancel="up"
        @lostpointercapture="up"
        @keydown="keydown(key, $event)"
        @keyup="keyup(key, $event)"
      >
        <title>{{ tooltip(key) }}</title>
        <rect v-if="key.style !== 'nav'" class="lip" :x="key.x" :y="key.y + 7" :width="key.width" :height="key.height" :rx="key.round ? key.width / 2 : key.height / 2" />
        <rect v-if="key.style !== 'nav'" class="cap" :x="key.x" :y="key.y" :width="key.width" :height="key.height" :rx="key.round ? key.width / 2 : key.height / 2" />
        <rect v-else class="hit" :x="key.x" :y="key.y" :width="key.width" :height="key.height" rx="43" />
        <rect v-if="mapped(key.name)" class="glow" :x="key.x" :y="key.y" :width="key.width" :height="key.height" :rx="key.round ? key.width / 2 : key.style === 'nav' ? 43 : key.height / 2" />
        <path v-if="key.glyph" class="glyph" :d="key.glyph" :transform="`translate(${key.x + key.width / 2 - 30} ${key.y + key.height / 2 - 30}) scale(2.5)`" />
        <template v-else>
          <text v-if="key.shift" class="legend shift" :x="key.x + 14" :y="key.y + 29" text-anchor="start">{{ key.shift }}</text>
          <text v-if="key.alpha" class="legend alpha" :x="key.x + key.width - 14" :y="key.y + 29" text-anchor="end">{{ key.alpha }}</text>
          <text class="label" :style="{ fontSize: labelSize(key) + 'px' }" :x="key.x + key.width / 2" :y="labelY(key)" text-anchor="middle">{{ key.label }}</text>
        </template>
      </g>
    </svg>
    <div v-if="mode === 'play'" class="screen" :style="ui.large ? null : screenStyle"><ScreenView /></div>
  </div>
</template>

<style scoped>
.calc { position: relative; width: min(100cqw, calc(100cqh * var(--ratio))); aspect-ratio: var(--ratio); user-select: none; touch-action: manipulation; }
svg { width: 100%; height: 100%; display: block; overflow: visible; }
.body { fill: #fbfbfb; stroke: #d6d6d8; stroke-width: 4; }
.brand { font: 700 42px system-ui, sans-serif; letter-spacing: 0.22em; text-transform: uppercase; fill: #555; }
.bezel { fill: #ededee; stroke: #d6d6d8; stroke-width: 3; }
.cross rect { fill: #e9e9eb; }
.cross .cross-lip { fill: #c4c4c8; }
.key .lip { fill: #c6c6ca; }
.key.home .lip { fill: #4d7d22; }
.key.power .lip { fill: #000; }
.screen { position: absolute; border-radius: 2px; overflow: hidden; }
/* Mode « grand » : seul l'écran, le plus grand possible, dans un cadre noir comme celui de la calculatrice. */
.calc.large { width: 100%; height: 100%; aspect-ratio: auto; display: grid; place-items: center; }
.calc.large .screen { position: static; width: min(100cqw - 24px, (100cqh - 24px) * 4 / 3); aspect-ratio: 4 / 3; border: 12px solid #111; border-radius: 18px; box-sizing: content-box; box-shadow: var(--shadow); background: #000; }
.key { cursor: pointer; outline: none; }
.key .cap { fill: url(#key-grad); stroke: #cfcfd2; stroke-width: 3; }
.key .label { font: 600 40px system-ui, sans-serif; fill: #2a2a2e; pointer-events: none; }
.key .legend { font: 600 26px system-ui, sans-serif; pointer-events: none; }
.key .legend.shift { fill: #4d8a1f; }
.key .legend.alpha { fill: #8a8a90; }
.key.mod .label { fill: #4d8a1f; }
.key.nav .label { fill: #55555b; font-size: 38px; }
.key.nav .hit { fill: transparent; }
.key.nav:hover .hit { fill: rgb(0 0 0 / 6%); }
.key.home .cap { fill: #74ad3a; stroke: #5b8f27; }
.key.power .cap { fill: #242426; stroke: #111; }
.key .glyph { fill: none; stroke: #2a2a2e; stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round; pointer-events: none; }
.key.home .glyph, .key.power .glyph { stroke: #fff; }
.key:hover .cap { fill: #f6f6f7; }
.key:active .cap { transform: translateY(4px); }
.key:focus-visible .cap, .key:focus-visible .hit { stroke: var(--focus); stroke-width: 8; }
.key .glow { fill: #3ddc84; fill-opacity: 0.35; stroke: #1fb865; stroke-width: 5; pointer-events: none; }
.key.selected .cap, .key.selected .hit { stroke: #2b6cb0; stroke-width: 8; }
.map .key .glow { fill-opacity: 0.45; }
</style>
