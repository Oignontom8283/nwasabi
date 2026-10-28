<script setup lang="ts">
// L'onglet Débogage : tous les outils pour comprendre ce que fait une application, sous une même barre de navigation.
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ui } from "../../ui.js";
import type { DebugView } from "../../ui.js";
import { gdb } from "../../gdbBridge.js";
import { server } from "../../serverLink.js";
import { useEmu } from "../../emulator.js";
import { refreshDebugInfo } from "../../sourceStore.js";
import SourceView from "../debug/SourceView.vue";
import GdbView from "../debug/GdbView.vue";
import CodeView from "../debug/CodeView.vue";
import ConsoleView from "../debug/ConsoleView.vue";
import PointsView from "../debug/PointsView.vue";
import DebugInfo from "../debug/DebugInfo.vue";
import ProfileCard from "../ProfileCard.vue";
import SvcLogCard from "../SvcLogCard.vue";
import WatchPoints from "../WatchPoints.vue";

const { t } = useI18n();
// La vue GDB passe par le serveur Nwasabi : sur une page hébergée ailleurs (site statique), elle n'est pas proposée.
const VIEWS = computed<DebugView[]>(() => {
  const views: DebugView[] = ["code", "source", "points", "watch", "console", "profile", "calls"];
  return server.active ? [...views, "gdb"] : views;
});
const view = computed(() => (VIEWS.value.includes(ui.debugView) ? ui.debugView : "code"));
const info = ref(false);

// Les informations de débogage du code source se relisent à chaque nouvelle application : la vue Source en dépend.
const emulator = useEmu();
watch(
  () => [emulator.state.appName, emulator.state.deviceSeq, emulator.state.loaded] as const,
  ([name, , loaded]) => void (loaded ? refreshDebugInfo(emulator, name) : undefined),
  { immediate: true },
);
</script>

<template>
  <div class="debug">
    <header>
      <div class="views" role="tablist" :aria-label="t('debug.views.label')">
        <button v-for="v in VIEWS" :key="v" role="tab" class="view" :class="{ active: view === v }" :aria-selected="view === v" @click="ui.debugView = v">
          {{ t(`debug.views.${v}`) }}
        </button>
      </div>
      <button class="icon about" :title="t('debugInfo.open')" :aria-label="t('debugInfo.open')" @click="info = true">i</button>
    </header>

    <p v-if="gdb.enabled && gdb.clients > 0" class="banner" role="status">{{ t("gdb.driven") }}</p>
    <p v-if="['source', 'code', 'points', 'watch', 'console'].includes(view)" class="intro muted">{{ t(`debug.intro.${view}`) }}</p>

    <CodeView v-if="view === 'code'" />
    <SourceView v-else-if="view === 'source'" />
    <PointsView v-else-if="view === 'points'" />
    <WatchPoints v-else-if="view === 'watch'" />
    <ConsoleView v-else-if="view === 'console'" />
    <ProfileCard v-else-if="view === 'profile'" />
    <SvcLogCard v-else-if="view === 'calls'" />
    <GdbView v-else />

    <DebugInfo v-if="info" @close="info = false" />
  </div>
</template>

<style scoped>
.debug { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; align-content: start; }
header { display: flex; align-items: center; gap: 8px; }
.views { display: flex; gap: 2px; padding: 2px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface-2); flex: 1; overflow-x: auto; scrollbar-width: none; }
.view { border: none; background: transparent; padding: 4px 12px; border-radius: 6px; font-size: 13px; color: var(--muted); white-space: nowrap; }
.view:hover { color: var(--text); }
.view.active { background: var(--surface); color: var(--text); font-weight: 600; box-shadow: 0 0 0 1px var(--line); }
.about { width: 26px; height: 26px; padding: 0; border-radius: 50%; font-weight: 700; font-family: Georgia, serif; font-style: italic; flex: none; display: inline-grid; place-items: center; line-height: 1; }
.about::first-line { line-height: 1; }
.banner { margin: 0; padding: 6px 10px; border: 1px solid var(--warn); border-radius: 8px; background: color-mix(in srgb, var(--warn) 14%, transparent); font-size: 12.5px; }
.intro { margin: 0; font-size: 12.5px; }
</style>
