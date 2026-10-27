<script setup lang="ts">
// L'onglet Débogage : tous les outils pour comprendre ce que fait une application, sous une même barre de navigation.
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { ui } from "../../ui.js";
import type { DebugView } from "../../ui.js";
import CodeView from "../debug/CodeView.vue";
import ConsoleView from "../debug/ConsoleView.vue";
import PointsView from "../debug/PointsView.vue";
import DebugInfo from "../debug/DebugInfo.vue";
import ProfileCard from "../ProfileCard.vue";
import SvcLogCard from "../SvcLogCard.vue";
import WatchPoints from "../WatchPoints.vue";

const { t } = useI18n();
const VIEWS: DebugView[] = ["code", "points", "watch", "console", "profile", "calls"];
const info = ref(false);
</script>

<template>
  <div class="debug">
    <header>
      <div class="views" role="tablist" :aria-label="t('debug.views.label')">
        <button v-for="v in VIEWS" :key="v" role="tab" class="view" :class="{ active: ui.debugView === v }" :aria-selected="ui.debugView === v" @click="ui.debugView = v">
          {{ t(`debug.views.${v}`) }}
        </button>
      </div>
      <button class="icon about" :title="t('debugInfo.open')" :aria-label="t('debugInfo.open')" @click="info = true">i</button>
    </header>

    <p v-if="['code', 'points', 'watch', 'console'].includes(ui.debugView)" class="intro muted">{{ t(`debug.intro.${ui.debugView}`) }}</p>

    <CodeView v-if="ui.debugView === 'code'" />
    <PointsView v-else-if="ui.debugView === 'points'" />
    <WatchPoints v-else-if="ui.debugView === 'watch'" />
    <ConsoleView v-else-if="ui.debugView === 'console'" />
    <ProfileCard v-else-if="ui.debugView === 'profile'" />
    <SvcLogCard v-else />

    <DebugInfo v-if="info" @close="info = false" />
  </div>
</template>

<style scoped>
.debug { display: grid; gap: 10px; align-content: start; }
header { display: flex; align-items: center; gap: 8px; }
.views { display: flex; gap: 2px; padding: 2px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface-2); flex: 1; overflow-x: auto; scrollbar-width: none; }
.view { border: none; background: transparent; padding: 4px 12px; border-radius: 6px; font-size: 13px; color: var(--muted); white-space: nowrap; }
.view:hover { color: var(--text); }
.view.active { background: var(--surface); color: var(--text); font-weight: 600; box-shadow: 0 0 0 1px var(--line); }
.about { width: 26px; height: 26px; border-radius: 50%; font-weight: 700; font-family: Georgia, serif; font-style: italic; flex: none; }
.intro { margin: 0; font-size: 12.5px; }
</style>
