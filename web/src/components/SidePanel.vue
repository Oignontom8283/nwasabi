<script setup>
import { computed, onBeforeUnmount } from "vue";
import { useI18n } from "vue-i18n";
import { PANEL_MIN, PANEL_ROOM, panelPixels, TABS, ui } from "../ui.js";
import Icon from "./Icon.vue";
import FilesTab from "./tabs/FilesTab.vue";
import KeysTab from "./tabs/KeysTab.vue";
import MemoryTab from "./tabs/MemoryTab.vue";
import SettingsTab from "./tabs/SettingsTab.vue";
import StructuresTab from "./tabs/StructuresTab.vue";
import TraceTab from "./tabs/TraceTab.vue";

const { t } = useI18n();
const COMPONENTS = { files: FilesTab, memory: MemoryTab, structures: StructuresTab, keys: KeysTab, settings: SettingsTab, trace: TraceTab };
const ICONS = { files: "folder", memory: "chip", structures: "layers", keys: "keyboard", settings: "sliders", trace: "activity" };

const width = computed(() => panelPixels());

// Redimensionnement à la souris (ou au doigt) par la poignée de gauche, et au clavier. Pendant le glissement, la
// largeur n'est appliquée qu'une fois par image : suivre chaque événement du pointeur fait inutilement du travail.
let dragging = false;
let wanted = 0;
let frame = 0;
function startDrag(event) {
  dragging = true;
  event.currentTarget.setPointerCapture(event.pointerId);
}
function drag(event) {
  if (!dragging) return;
  wanted = ui.winWidth - event.clientX;
  frame ||= requestAnimationFrame(() => {
    frame = 0;
    ui.panelWidth = Math.max(PANEL_MIN, Math.min(wanted, ui.winWidth - PANEL_ROOM));
  });
}
function endDrag() {
  dragging = false;
}
function resizeKey(event) {
  const step = event.shiftKey ? 80 : 24;
  if (event.key === "ArrowLeft") ui.panelWidth = Math.min(width.value + step, ui.winWidth - PANEL_ROOM);
  else if (event.key === "ArrowRight") ui.panelWidth = Math.max(width.value - step, PANEL_MIN);
  else if (event.key === "Home") ui.panelWidth = ui.winWidth - PANEL_ROOM;
  else if (event.key === "End") ui.panelWidth = PANEL_MIN;
  else return;
  event.preventDefault();
}
onBeforeUnmount(() => cancelAnimationFrame(frame));

function tabKey(event) {
  const i = TABS.indexOf(ui.tab);
  const to = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: TABS.length - 1 }[event.key];
  if (to === undefined) return;
  ui.tab = TABS[(to + TABS.length) % TABS.length];
  event.preventDefault();
  document.getElementById(`tab-${ui.tab}`)?.focus();
}
</script>

<template>
  <section class="side" :style="{ width: width + 'px' }" :aria-label="t('panel.label')">
    <div
      class="grip"
      role="separator"
      aria-orientation="vertical"
      tabindex="0"
      :aria-label="t('panel.resize')"
      :aria-valuenow="width"
      :title="t('panel.resizeHint')"
      @pointerdown="startDrag"
      @pointermove="drag"
      @pointerup="endDrag"
      @pointercancel="endDrag"
      @keydown="resizeKey"
    />
    <div class="strip">
      <div class="tabs" role="tablist" :aria-label="t('panel.tabs')" @keydown="tabKey">
        <button
          v-for="id in TABS"
          :id="`tab-${id}`"
          :key="id"
          role="tab"
          class="tab"
          :class="{ active: ui.tab === id }"
          :aria-selected="ui.tab === id"
          :aria-controls="`panel-${id}`"
          :tabindex="ui.tab === id ? 0 : -1"
          @click="ui.tab = id"
        >
          <Icon :name="ICONS[id]" /> {{ t(`tabs.${id}`) }}
        </button>
      </div>
      <button class="collapse icon" :title="t('panel.collapse')" :aria-label="t('panel.collapse')" @click="ui.panelOpen = false"><Icon name="right" /></button>
    </div>
    <div :id="`panel-${ui.tab}`" class="body" role="tabpanel" :aria-labelledby="`tab-${ui.tab}`" tabindex="-1">
      <component :is="COMPONENTS[ui.tab]" />
    </div>
  </section>
</template>

<style scoped>
.side { contain: layout; position: absolute; right: 0; top: 0; bottom: 0; display: flex; flex-direction: column; background: var(--surface); border-left: 1px solid var(--line); box-shadow: var(--shadow); z-index: 10; }
.grip { position: absolute; left: -4px; top: 0; bottom: 0; width: 9px; cursor: col-resize; z-index: 3; touch-action: none; }
.grip:hover, .grip:focus-visible, .grip:active { background: var(--accent-soft); }
.strip { display: flex; align-items: flex-end; gap: 6px; padding: 6px 6px 0; background: var(--surface-2); border-bottom: 1px solid var(--line); }
.tabs { display: flex; gap: 2px; min-width: 0; overflow-x: auto; flex: 1; scrollbar-width: none; }
.tab {
  border: 1px solid var(--line); border-bottom: none; border-radius: 6px 6px 0 0; padding: 4px 10px; font-size: 13px; background: var(--key); color: var(--muted);
  margin-bottom: -1px; position: relative; top: 1px;
}
.tab:hover { color: var(--text); }
.tab.active { background: var(--surface); color: var(--text); font-weight: 600; box-shadow: inset 0 2px 0 var(--accent); }
.collapse { margin-bottom: 4px; }
.body { flex: 1; min-height: 0; overflow: auto; padding: 10px 12px; }
.body:focus { outline: none; }
</style>
