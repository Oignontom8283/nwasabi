<script setup lang="ts">
// Le débogueur GDB : brancher un débogueur externe (GDB, LLDB, VS Code…) sur l'émulation de cette page.
import { onBeforeUnmount, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../../emulator.js";
import { gdb, probeGdb, setGdb } from "../../gdbBridge.js";
import { poll } from "../../poll.js";
import { checkedOf, download } from "../../util.js";
import { source } from "../../sourceStore.js";

const { t } = useI18n();
const emulator = useEmu();
let stopPolling = () => {};
onMounted(() => (stopPolling = poll(probeGdb, 2000)));
onBeforeUnmount(() => stopPolling());
const address = () => `127.0.0.1:${gdb.port}`;
const elfName = () => `${emulator.state.appName.replace(/\.nwa$/i, "") || "application"}.elf`;
/** L'application liée en ELF : GDB y lit les symboles, les lignes de source et les variables (`file application.elf`). */
async function exportElf() {
  const reply = await emulator.quiet({ cmd: "export_elf" });
  if (!reply.ok) return;
  download(elfName(), Uint8Array.from(atob(String(reply.elf)), (c) => c.charCodeAt(0)));
}
</script>

<template>
  <div class="gdb">
    <p class="muted intro">{{ t("gdb.intro") }}</p>

    <section v-if="!gdb.available" class="card">
      <h3 class="subhead">{{ t("gdb.unavailable") }}</h3>
      <p>{{ t("gdb.howToStart") }}</p>
      <pre class="mono">nwasabi --nwa jeu.nwa --gdb 3333</pre>
    </section>

    <template v-else>
      <section class="card">
        <label class="switch">
          <input type="checkbox" :checked="gdb.enabled" @change="setGdb(emulator, checkedOf($event))" />
          <span>{{ t("gdb.enable") }}</span>
        </label>
        <p class="muted small">{{ t("gdb.enableHelp") }}</p>
        <p v-if="gdb.error" class="error" role="alert">{{ gdb.error }}</p>
      </section>

      <section class="card" :class="{ off: !gdb.enabled }">
        <h3 class="subhead">{{ t("gdb.connect") }}</h3>
        <pre class="mono">(gdb) file {{ elfName() }}
(gdb) target remote {{ address() }}</pre>
        <p class="small"><button @click="exportElf">{{ t("gdb.exportElf") }}</button> <span class="muted">{{ source.available ? t("gdb.elfHelp") : t("gdb.elfNoDebug") }}</span></p>
        <p class="status" role="status">
          <span class="dot" :class="{ on: gdb.clients > 0 }" aria-hidden="true" />
          <template v-if="!gdb.enabled">{{ t("gdb.statusOff") }}</template>
          <template v-else-if="gdb.clients > 0">{{ t("gdb.statusConnected", { n: gdb.clients }, gdb.clients) }}</template>
          <template v-else>{{ t("gdb.statusWaiting") }}</template>
        </p>
        <p class="muted small">{{ t("gdb.note") }}</p>
      </section>
    </template>
  </div>
</template>

<style scoped>
.gdb { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; align-content: start; }
.intro { margin: 0; font-size: 12.5px; }
.card { padding: 10px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface-2); display: grid; gap: 6px; }
.card.off { opacity: 0.6; }
.card p { margin: 0; }
.small { font-size: 12px; }
.switch { display: flex; gap: 8px; align-items: center; font-weight: 600; }
pre { margin: 0; padding: 6px 8px; border: 1px solid var(--line); border-radius: 6px; background: var(--surface); overflow-x: auto; }
.status { display: flex; gap: 8px; align-items: center; font-size: 13px; }
.dot { width: 9px; height: 9px; border-radius: 50%; background: var(--muted); }
.dot.on { background: var(--ok); }
</style>
