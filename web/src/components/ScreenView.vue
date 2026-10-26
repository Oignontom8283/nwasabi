<script setup>
import { inject, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { screenApi, ui } from "../ui.js";
import { download, stem } from "../util.js";
import Icon from "./Icon.vue";

const { t } = useI18n();
const emulator = inject("emulator");
const pick = inject("pick");
const { state } = emulator;
const wrap = ref(null);
const canvas = ref(null);
const showClose = ref(false);
let hideTimer = 0;

onMounted(() => {
  emulator.attach(canvas.value, wrap.value);
  document.addEventListener("fullscreenchange", onChange);
  screenApi.toggleFullscreen = toggle;
  screenApi.exitFullscreen = exit;
  screenApi.capture = capture;
});
onBeforeUnmount(() => {
  document.removeEventListener("fullscreenchange", onChange);
  clearTimeout(hideTimer);
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
function onMove(event) {
  if (!ui.fullscreen) return;
  const inZone = event.clientY < 90 && Math.abs(event.clientX - innerWidth / 2) < 160;
  if (inZone) {
    showClose.value = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (showClose.value = false), 2500);
  }
}

// La capture est faite par le site : le cœur ne fournit que les pixels, le navigateur les encode.
function capture() {
  canvas.value?.toBlob((blob) => {
    if (blob) download(`${stem(state.appName || "nwasabi")}.png`, blob);
  }, "image/png");
}
</script>

<template>
  <div ref="wrap" class="screen-wrap" :class="{ full: ui.fullscreen }" @pointermove="onMove">
    <canvas ref="canvas" width="320" height="240" role="img" :aria-label="t('screen.label')" />
    <div v-if="!state.loaded && !ui.fullscreen" class="drop" :class="{ data: state.needsData }" @click="pick(state.needsData ? 'data' : 'app')">
      <Icon name="upload" :size="44" />
      <strong>{{ state.needsData ? t("screen.dropData") : t("screen.dropApp") }}</strong>
      <span>{{ state.needsData ? t("screen.dropDataSub") : t("screen.dropAppSub") }}</span>
      <button v-if="state.needsData" class="primary" @click.stop="emulator.runWithoutData()">{{ t("screen.runWithout") }}</button>
    </div>
    <button v-if="ui.fullscreen" class="close" :class="{ shown: showClose }" :aria-label="t('screen.exitFullscreen')" :title="t('screen.exitFullscreen')" @click="exit"><Icon name="close" /> {{ t("screen.exitFullscreen") }}</button>
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
.drop:hover { border-color: #8cc152; color: #fff; }
.drop strong { font-weight: 600; }
.drop span { font-size: 0.8em; color: #8d939c; }
</style>
