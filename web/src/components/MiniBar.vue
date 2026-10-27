<script setup lang="ts">
import { useEmu } from "../emulator.js";
import { computed, inject, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { recording, screenApi, ui } from "../ui.js";
import { keyed } from "../shortcuts.js";
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

// Le temps de l'application vient du Worker, qui le renvoie même sans nouvelle image.
const timeMs = computed(() => (state.loaded ? state.timeMs : 0));

const mhz = computed(() => (state.limitMhz ? `${state.limitMhz} MHz` : t("mini.noLimit")));
</script>

<template>
  <aside class="mini" :aria-label="t('mini.label')">
    <button class="tools" :title="keyed(t('mini.openPanel'), 'panel')" @click="ui.panelOpen = true"><Icon name="panel" :size="16" /> {{ t("mini.tools") }}</button>
    <dl>
      <div :title="t('mini.statusHint')">
        <dt>{{ t("mini.status") }}</dt>
        <dd><span class="dot" :class="status" aria-hidden="true" /> {{ t(`mini.states.${status}`) }}</dd>
      </div>
      <div :title="t('mini.framesHint')"><dt>{{ t("mini.frames") }}</dt><dd class="mono">{{ n(state.frames) }}</dd></div>
      <div :title="t('mini.timeHint')"><dt>{{ t("mini.time") }}</dt><dd class="mono">{{ formatDuration(timeMs) }}</dd></div>
      <div :title="t('mini.speedHint')"><dt>{{ t("mini.speed") }}</dt><dd class="mono">{{ mhz }}</dd></div>
      <div v-if="state.running" :title="state.slow ? t('mini.slowHint') : t('mini.rateHint')" :class="{ slow: state.slow }">
        <dt>{{ t("mini.rate") }}</dt><dd class="mono">{{ state.mips }} MIPS<template v-if="state.slow"> · {{ t("mini.slow") }}</template></dd>
      </div>
      <div v-if="state.stop?.kind === 'fault'" :title="t('mini.faultHint')" class="fault"><dt>{{ t("mini.fault") }}</dt><dd>{{ state.stop.fault }}</dd></div>
    </dl>
    <button :disabled="!state.loaded" :title="keyed(t('mini.captureHint'), 'capture')" @click="screenApi.capture()"><Icon name="camera" /> {{ t("mini.capture") }}</button>
    <button :disabled="!state.loaded" :title="keyed(t('mini.copyHint'), 'copy')" @click="screenApi.copy()"><Icon name="copy" /> {{ t("mini.copy") }}</button>
    <button :disabled="!state.loaded" :aria-pressed="recording.on" :class="{ rec: recording.on }" :title="keyed(recording.on ? t('mini.recordStopHint') : t('mini.recordHint'), 'gif')" @click="screenApi.toggleRecording()"><Icon name="dot" /> {{ recording.on ? t("mini.recordStop") : t("mini.record") }}</button>
    <button :aria-pressed="ui.large" :title="keyed(t('mini.largeHint'), 'large')" @click="ui.large = !ui.large"><Icon name="expand" /> {{ t("mini.large") }}</button>
    <button :disabled="!state.loaded" :title="keyed(t('mini.fullscreenHint'), 'fullscreen')" @click="screenApi.toggleFullscreen()"><Icon name="maximize" /> {{ t("mini.fullscreen") }}</button>
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
.rec :deep(svg) { color: var(--danger); }
.slow dd { color: var(--warn); }
.hide { justify-self: end; padding: 0 8px; }
button { justify-content: center; }
</style>
