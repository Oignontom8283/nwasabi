<script setup lang="ts">
import { useEmu } from "../emulator.js";
import { computed, inject, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { notify, recording, screenApi, ui, usePick } from "../ui.js";
import { download, stem } from "../util.js";
import { poll } from "../poll.js";
import Icon from "./Icon.vue";

const { t } = useI18n();
const emulator = useEmu();
const pick = usePick();
const { state } = emulator;
const wrap = ref<HTMLElement | null>(null);
const canvas = ref<HTMLCanvasElement | null>(null);
const showClose = ref(false);
const over = ref(false); // un fichier est glissé au-dessus de la zone de dépôt
/** Luminosité réglée (0 à 240, par pas de 16) : l'application et les Réglages la modifient. */
const brightness = ref(240);
let stopBrightness = () => {};
// Une luminosité nulle laisse l'écran à peine lisible, comme sur l'appareil, au lieu de le noircir tout à fait.
const screenFilter = computed(() => (ui.backlight ? `brightness(${(0.2 + 0.8 * (brightness.value / 240)).toFixed(3)})` : "none"));
let hideTimer: ReturnType<typeof setTimeout> | undefined;

onMounted(() => {
  if (canvas.value && wrap.value) emulator.attach(canvas.value, wrap.value);
  document.addEventListener("fullscreenchange", onChange);
  stopBrightness = poll(async () => {
    if (!state.loaded || !ui.backlight) return;
    const reply = await emulator.quiet({ cmd: "settings_get" });
    if (reply.ok) brightness.value = Number(reply.brightness);
  }, 500);
  screenApi.toggleFullscreen = toggle;
  screenApi.exitFullscreen = exit;
  screenApi.capture = capture;
  screenApi.copy = copy;
  screenApi.toggleRecording = toggleRecording;
});
onBeforeUnmount(() => {
  document.removeEventListener("fullscreenchange", onChange);
  clearTimeout(hideTimer);
  stopBrightness();
});

function onChange() {
  ui.fullscreen = document.fullscreenElement === wrap.value;
  showClose.value = false;
}

async function toggle() {
  if (document.fullscreenElement) await exit();
  else await wrap.value?.requestFullscreen?.().catch(() => {});
}

async function exit() {
  if (document.fullscreenElement) await document.exitFullscreen().catch(() => {});
}

// Un petit bouton « fermer » apparaît quand la souris passe en haut au milieu de l'écran plein.
function onMove(event: PointerEvent) {
  if (!ui.fullscreen) return;
  const inZone = event.clientY < 90 && Math.abs(event.clientX - innerWidth / 2) < 160;
  if (inZone) {
    showClose.value = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (showClose.value = false), 2500);
  }
}

// L'écran est dessiné par le Worker de l'émulation : c'est lui qui encode l'image, la page ne fait que l'enregistrer.
async function capture() {
  const blob = await emulator.capture();
  if (blob) download(`${stem(state.appName || "nwasabi")}.png`, blob);
}

async function copy() {
  const blob = await emulator.capture();
  try {
    if (!blob) throw new Error("vide");
    await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
    notify(t("screen.copied"));
  } catch {
    notify(t("screen.copyFailed"));
  }
}

async function toggleRecording() {
  if (!recording.on) {
    recording.on = await emulator.startRecording();
    recording.since = Date.now();
    return;
  }
  recording.on = false;
  const gif = await emulator.stopRecording();
  if (gif) download(`${stem(state.appName || "nwasabi")}.gif`, new Blob([gif as BlobPart], { type: "image/gif" }));
  else notify(t("screen.recordEmpty"));
}
</script>

<template>
  <div ref="wrap" class="screen-wrap" :class="{ full: ui.fullscreen }" @pointermove="onMove">
    <canvas ref="canvas" width="320" height="240" role="img" :aria-label="t('screen.label')" :style="{ filter: screenFilter }" />
    <div
      v-if="!state.loaded && !ui.fullscreen"
      class="drop"
      :class="{ data: state.needsData, over }"
      role="button"
      tabindex="0"
      @pointerdown.stop
      @click="pick(state.needsData ? 'data' : 'app')"
      @keydown.enter.prevent="pick(state.needsData ? 'data' : 'app')"
      @dragenter.prevent="over = true"
      @dragover.prevent="over = true"
      @dragleave="(e: DragEvent) => (e.currentTarget as Element).contains(e.relatedTarget as Node | null) || (over = false)"
      @drop="over = false"
    >
      <Icon name="upload" :size="44" />
      <strong>{{ state.needsData ? t("screen.dropData") : t("screen.dropApp") }}</strong>
      <span>{{ state.needsData ? t("screen.dropDataSub") : t("screen.dropAppSub") }}</span>
      <button v-if="state.needsData" class="primary" @click.stop="emulator.runWithoutData()">{{ t("screen.runWithout") }}</button>
    </div>
    <!-- pointerdown.stop : sinon l'écran capte le pointeur (gestes tactiles) et le clic n'arrive jamais au bouton. -->
    <button v-if="ui.fullscreen" class="close" :class="{ shown: showClose }" @pointerdown.stop :aria-label="t('screen.exitFullscreen')" :title="t('screen.exitFullscreen')" @click="exit"><Icon name="close" /> {{ t("screen.exitFullscreen") }}</button>
    <p v-if="ui.fullscreen" class="sr-only">{{ t("screen.exitHint") }}</p>
  </div>
</template>

<style scoped>
.screen-wrap { position: relative; width: 100%; height: 100%; background: #000; touch-action: none; user-select: none; container-type: size; }
canvas { width: 100%; height: 100%; display: block; image-rendering: pixelated; touch-action: none; }
.screen-wrap.full { display: grid; place-items: center; background: #000; }
.screen-wrap.full canvas { width: min(100vw, 133.333vh); height: auto; aspect-ratio: 4 / 3; }
.close {
  position: absolute; top: 14px; left: 50%; transform: translateX(-50%); opacity: 0; pointer-events: none;
  transition: opacity 0.15s; border-radius: 999px; padding: 6px 14px; background: rgb(0 0 0 / 70%); color: #fff; border-color: #555;
}
.close.shown, .close:focus-visible { opacity: 1; pointer-events: auto; }
.drop {
  position: absolute; inset: 6%; display: grid; place-content: center; justify-items: center; gap: 2.5cqh; text-align: center; color: #c9cdd3;
  border: 2px dashed #5b616b; border-radius: 8px; background: #16181c; cursor: pointer; font-size: clamp(11px, 5.2cqw, 20px);
}
.drop { transition: border-color 0.12s, background 0.12s; }
.drop:hover, .drop:focus-visible { border-color: #8cc152; color: #fff; }
/* Un fichier est au-dessus : traits plus épais et fond teinté, pour montrer qu'on peut le lâcher ici. */
.drop.over { border-width: 4px; border-color: #8cc152; background: #1f2a17; color: #fff; }
.drop > * { pointer-events: none; }
.drop > button { pointer-events: auto; }
.drop strong { font-weight: 600; }
.drop span { font-size: 0.8em; color: #8d939c; }
</style>
