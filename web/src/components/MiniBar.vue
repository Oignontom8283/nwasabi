<script setup lang="ts">
import { useEmu } from "../emulator.js";
import { computed, inject, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { screenApi, ui } from "../ui.js";
import { formatDuration } from "../util.js";
import Icon from "./Icon.vue";

const { t, n } = useI18n();
const emulator = useEmu();
const { state } = emulator;

const status = computed(() => {
  if (!state.loaded) return "idle";
  if (state.running) return "running";
  if (state.stop?.kind === "exited") return "exited";
  if (state.stop?.kind === "fault") return "fault";
  return "paused";
});

// Le temps de l'application est lu à intervalles réguliers : il continue de défiler même sans nouvelle image.
const timeMs = ref(0);
let timer = 0;
onMounted(() => {
  timer = setInterval(() => (timeMs.value = state.loaded ? emulator.time() : 0), 500);
});
onBeforeUnmount(() => clearInterval(timer));

const mhz = computed(() => (state.limitMhz ? `${state.limitMhz} MHz` : t("mini.noLimit")));
</script>

<template>
  <aside class="mini" :aria-label="t('mini.label')">
    <button class="tools" :title="t('mini.openPanel')" @click="ui.panelOpen = true"><Icon name="panel" :size="16" /> {{ t("mini.tools") }}</button>
    <dl>
      <div :title="t('mini.statusHint')">
        <dt>{{ t("mini.status") }}</dt>
        <dd><span class="dot" :class="status" aria-hidden="true" /> {{ t(`mini.states.${status}`) }}</dd>
      </div>
      <div :title="t('mini.framesHint')"><dt>{{ t("mini.frames") }}</dt><dd class="mono">{{ n(state.frames) }}</dd></div>
      <div :title="t('mini.timeHint')"><dt>{{ t("mini.time") }}</dt><dd class="mono">{{ formatDuration(timeMs) }}</dd></div>
      <div :title="t('mini.speedHint')"><dt>{{ t("mini.speed") }}</dt><dd class="mono">{{ mhz }}</dd></div>
      <div v-if="state.stop?.kind === 'fault'" :title="t('mini.faultHint')" class="fault"><dt>{{ t("mini.fault") }}</dt><dd>{{ state.stop.fault }}</dd></div>
    </dl>
    <button :disabled="!state.loaded" :title="t('mini.captureHint')" @click="screenApi.capture()"><Icon name="camera" /> {{ t("mini.capture") }}</button>
    <button :aria-pressed="ui.large" :title="t('mini.largeHint')" @click="ui.large = !ui.large"><Icon name="expand" /> {{ t("mini.large") }}</button>
    <button :disabled="!state.loaded" :title="t('mini.fullscreenHint')" @click="screenApi.toggleFullscreen()"><Icon name="maximize" /> {{ t("mini.fullscreen") }}</button>
    <button class="hide" :title="t('mini.hideHint')" :aria-label="t('mini.hideHint')" @click="ui.miniVisible = false"><Icon name="right" /></button>
  </aside>
</template>

<style scoped>
.mini {
  position: absolute; right: 12px; top: 12px; width: 168px; display: grid; gap: 6px; padding: 8px; z-index: 10;
  background: var(--surface); border: 1px solid var(--line); border-radius: 8px; box-shadow: var(--shadow); font-size: 12.5px;
}
dl { margin: 0; display: grid; gap: 6px; }
dl > div { display: flex; justify-content: space-between; gap: 8px; align-items: baseline; cursor: help; }
dt { color: var(--muted); font-size: 12px; }
dd { margin: 0; text-align: right; }
.dot { display: inline-block; width: 9px; height: 9px; border-radius: 50%; background: var(--muted); }
.dot.running { background: var(--ok); }
.dot.paused { background: var(--warn); }
.dot.fault { background: var(--danger); }
.fault dd { color: var(--danger); }
.hide { justify-self: end; padding: 0 8px; }
button { justify-content: center; }
</style>
