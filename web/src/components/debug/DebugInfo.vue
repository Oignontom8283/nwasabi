<script setup lang="ts">
// Fenêtre « i » du débogage : à quoi servent les outils, ce qu'ils ne font pas, et le lien vers la documentation complète.
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { server } from "../../serverLink.js";
import FloatingWindow from "../FloatingWindow.vue";

const emit = defineEmits<{ close: [] }>();
// La vue GDB n'existe que servie par Nwasabi.
const VIEWS = computed(() => (server.active ? ["code", "source", "points", "watch", "console", "profile", "calls", "gdb"] : ["code", "source", "points", "watch", "console", "profile", "calls"]));
const { t } = useI18n();

// Sur le site de la documentation, l'émulateur est à `/emulator/` et la page est à côté ; sinon, la page du dépôt.
const docs = location.pathname.includes("/emulator/")
  ? new URL("../guide/deboguer-emulateur", location.href).href
  : "https://github.com/Oignontom8283/nwasabi/blob/main/docs/guide/deboguer-emulateur.md";
</script>

<template>
  <Teleport to="body">
    <FloatingWindow :width="520" :title="t('debugInfo.title')" @close="emit('close')">
      <div class="info">
        <p>{{ t("debugInfo.intro") }}</p>
        <h3 class="subhead">{{ t("debugInfo.tools") }}</h3>
        <table>
          <thead><tr><th>{{ t("debugInfo.colView") }}</th><th>{{ t("debugInfo.colRole") }}</th></tr></thead>
          <tbody>
            <tr v-for="v in VIEWS" :key="v"><th scope="row">{{ t(`debug.views.${v}`) }}</th><td>{{ t(`debugInfo.${v}`) }}</td></tr>
          </tbody>
        </table>
        <h3 class="subhead">{{ t("debugInfo.limits") }}</h3>
        <ul>
          <li>{{ t("debugInfo.limit1") }}</li>
          <li>{{ t("debugInfo.limit2") }}</li>
          <li>{{ t("debugInfo.limit3") }}</li>
          <li>{{ t("debugInfo.limit4") }}</li>
        </ul>
        <p><a :href="docs" target="_blank" rel="noopener">{{ t("debugInfo.docs") }}</a></p>
      </div>
    </FloatingWindow>
  </Teleport>
</template>

<style scoped>
.info { display: grid; gap: 8px; font-size: 13.5px; line-height: 1.5; }
.info p, .info ul { margin: 0; }
.info ul { padding-left: 1.2em; display: grid; gap: 4px; }
table { border-collapse: collapse; width: 100%; font-size: 13px; }
th, td { text-align: left; vertical-align: top; padding: 5px 8px; border: 1px solid var(--line); }
thead th { background: var(--surface-2); font-size: 12px; }
tbody th { white-space: nowrap; font-weight: 600; }
a { color: var(--accent-strong); }
</style>
