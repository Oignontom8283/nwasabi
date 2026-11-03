<script setup lang="ts">
// La liste des raccourcis clavier, par thème : on la trouve dans la palette de commandes (Ctrl + K) ou avec la touche « ? ».
import { useI18n } from "vue-i18n";
import { SHORTCUTS } from "../shortcuts.js";
import FloatingWindow from "./FloatingWindow.vue";

const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();

const GROUPS: Array<{ id: string; rows: Array<[string, string]> }> = [
  {
    id: "general",
    rows: [
      ["palette", SHORTCUTS.palette],
      ["toggleRun", SHORTCUTS.toggleRun],
      ["frame", SHORTCUTS.frame],
      ["restart", SHORTCUTS.restart],
      ["quickSave", SHORTCUTS.quickSave],
      ["quickLoad", SHORTCUTS.quickLoad],
      ["reload", SHORTCUTS.reload],
    ],
  },
  { id: "screen", rows: [["capture", SHORTCUTS.capture], ["copy", SHORTCUTS.copy], ["gif", SHORTCUTS.gif], ["video", SHORTCUTS.video], ["fullscreen", SHORTCUTS.fullscreen], ["large", SHORTCUTS.large], ["panel", SHORTCUTS.panel]] },
  { id: "debug", rows: [["resume", "F5"], ["over", "F10"], ["step", "F11"], ["out", "Maj + F11"]] },
  { id: "lists", rows: [["move", "↑ ↓"], ["extend", "Maj + ↑ ↓"], ["all", "Ctrl + A"], ["copy", "Ctrl + C"], ["menu", "Maj + F10"], ["clear", "Échap"], ["breakpoint", "F9"], ["follow", "Entrée"]] },
];
</script>

<template>
  <Teleport to="body">
    <FloatingWindow :width="480" :title="t('shortcuts.title')" @close="emit('close')">
      <div class="sc">
        <section v-for="g in GROUPS" :key="g.id">
          <h3 class="subhead">{{ t(`shortcuts.groups.${g.id}`) }}</h3>
          <table>
            <tbody>
              <tr v-for="[id, keys] in g.rows" :key="id"><th scope="row">{{ t(`shortcuts.actions.${g.id}.${id}`) }}</th><td><kbd>{{ keys }}</kbd></td></tr>
            </tbody>
          </table>
        </section>
      </div>
    </FloatingWindow>
  </Teleport>
</template>

<style scoped>
.sc { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; font-size: 13.5px; }
table { border-collapse: collapse; width: 100%; }
th, td { padding: 3px 6px; border-bottom: 1px solid var(--line); text-align: left; font-weight: normal; }
td { text-align: right; white-space: nowrap; }
kbd { font: 12px ui-monospace, monospace; border: 1px solid var(--line); border-radius: 4px; padding: 1px 6px; background: var(--surface-2); }
</style>
