<script setup lang="ts">
// Instantanés : on photographie la RAM, on laisse l'application faire quelque chose, puis on compare pour voir quels octets ont
// changé. C'est la version « par plage » de la recherche de valeurs « modifiée / inchangée ».
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { compare } from "../snapshot.js";
import type { Change, Snapshot } from "../snapshot.js";
import { showInMemory } from "../ui.js";
import { hex, hex0x, hexToBytes } from "../util.js";
import type { HexRegion } from "./HexEditor.vue";
import AddressLink from "./AddressLink.vue";

const props = defineProps<{ regions: HexRegion[] }>();
const { t, locale } = useI18n();
const emulator = useEmu();

const CHUNK = 1 << 20;
const SHOWN = 300;
const snapshot = ref<Snapshot | null>(null);
const changes = ref<Change[]>([]);
const busy = ref(false);
const error = ref("");

/** Les zones modifiables lues par l'instantané. */
const rams = computed(() => props.regions.filter((r) => r.kind === "ram"));

async function readAll(): Promise<Array<{ base: number; bytes: Uint8Array }> | null> {
  const parts: Array<{ base: number; bytes: Uint8Array }> = [];
  for (const region of rams.value) {
    const bytes = new Uint8Array(region.size);
    for (let done = 0; done < region.size; done += CHUNK) {
      const reply = await emulator.quiet({ cmd: "memory_read", address: region.base + done, length: Math.min(CHUNK, region.size - done) });
      if (!reply.ok) {
        error.value = String(reply.error);
        return null;
      }
      bytes.set(hexToBytes(String(reply.hex)), done);
    }
    parts.push({ base: region.base, bytes });
  }
  return parts;
}

async function take() {
  busy.value = true;
  error.value = "";
  const parts = await readAll();
  if (parts) {
    snapshot.value = { takenAt: Date.now(), parts };
    changes.value = [];
  }
  busy.value = false;
}

async function run() {
  if (!snapshot.value) return;
  busy.value = true;
  error.value = "";
  const now = await readAll();
  if (now) changes.value = compare(snapshot.value, now);
  busy.value = false;
}

const total = computed(() => changes.value.reduce((n, c) => n + c.length, 0));
const bytesText = (bytes: Uint8Array) => Array.from(bytes.subarray(0, 8), (b) => hex(b, 2)).join(" ") + (bytes.length > 8 ? " …" : "");
const when = computed(() => (snapshot.value ? new Date(snapshot.value.takenAt).toLocaleTimeString(locale.value) : ""));
</script>

<template>
  <div class="snap">
    <p class="muted help">{{ t("snapshot.help") }}</p>
    <div class="bar">
      <button :disabled="busy || !emulator.state.loaded" @click="take">{{ snapshot ? t("snapshot.retake") : t("snapshot.take") }}</button>
      <button :disabled="busy || !snapshot" :title="t('snapshot.compareHint')" @click="run">{{ t("snapshot.compare") }}</button>
      <span v-if="snapshot" class="muted">{{ t("snapshot.taken", { time: when }) }}</span>
    </div>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <template v-if="snapshot && changes.length">
      <p class="muted">{{ t("snapshot.summary", { ranges: changes.length, bytes: total }, changes.length) }}</p>
      <div class="gh">
        <div class="gh-head cols" aria-hidden="true"><span>{{ t("snapshot.address") }}</span><span>{{ t("snapshot.size") }}</span><span>{{ t("snapshot.before") }}</span><span>{{ t("snapshot.after") }}</span></div>
        <div class="gh-rows">
          <div v-for="c in changes.slice(0, SHOWN)" :key="c.address" class="gh-row cols">
            <AddressLink :address="c.address" :length="c.length" />
            <span class="mono">{{ c.length }}</span>
            <span class="mono muted">{{ bytesText(c.before) }}</span>
            <span class="mono">{{ bytesText(c.after) }}</span>
          </div>
        </div>
      </div>
      <p v-if="changes.length > SHOWN" class="muted small">{{ t("snapshot.truncated", { n: SHOWN }) }}</p>
    </template>
    <p v-else-if="snapshot && !busy" class="muted">{{ t("snapshot.none") }}</p>
  </div>
</template>

<style scoped>
.snap { display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; }
.help { margin: 0; font-size: 12px; }
.bar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
.cols { grid-template-columns: 100px 52px minmax(0, 1fr) minmax(0, 1fr); }
.link { border: none; background: none; padding: 0; text-align: left; color: var(--accent-strong); cursor: pointer; }
.link:hover { text-decoration: underline; }
.gh-rows { max-height: 260px; overflow: auto; }
.small { margin: 0; font-size: 12px; }
</style>
