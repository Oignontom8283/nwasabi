<script setup lang="ts">
// Les signets : des raccourcis nommés vers des zones de la mémoire de l'application en cours (« score », « vies »…).
// Ils ne changent rien à l'application par eux-mêmes : ils servent à retrouver vite un endroit, à lire ce qu'il contient et,
// si l'on veut, à le modifier (comme dans la liste de la recherche de valeurs).
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { bookmarks, loadBookmarks, removeBookmark, renameBookmark, setBookmarkType, typeOf } from "../bookmarks.js";
import { SCAN_TYPES } from "../scan.js";
import type { ScanType } from "../scan.js";
import { memoryAccess } from "../scanStore.js";
import { hex0x } from "../util.js";
import { poll } from "../poll.js";
import AddressLink from "./AddressLink.vue";
import Icon from "./Icon.vue";
import ValueField from "./ValueField.vue";

const { t } = useI18n();
const emulator = useEmu();
const { state } = emulator;
const { readValue, writeValue } = memoryAccess(emulator);
watch(() => state.appName, (name) => loadBookmarks(state.loaded ? name : ""), { immediate: true });
watch(() => state.loaded, (loaded) => loadBookmarks(loaded ? state.appName : ""));

/** Valeur lue à chaque signet, par identifiant (`null` : illisible). */
const values = ref<Record<number, number | null>>({});
const message = ref("");

async function refresh() {
  if (!state.loaded) return;
  const next: Record<number, number | null> = {};
  for (const b of bookmarks.list) next[b.id] = await readValue(b.address, typeOf(b));
  if (JSON.stringify(next) !== JSON.stringify(values.value)) values.value = next;
}
let stopPolling = () => {};
onMounted(() => {
  stopPolling = poll(refresh, 400);
});
onBeforeUnmount(() => stopPolling());

async function write(id: number, value: number) {
  const b = bookmarks.list.find((x) => x.id === id);
  if (!b) return;
  message.value = (await writeValue(b.address, typeOf(b), value)) ? "" : t("scan.writeFailed", { address: hex0x(b.address) });
  await refresh();
}
const invalid = (id: number, text: string) => {
  const b = bookmarks.list.find((x) => x.id === id);
  message.value = t("scan.badValue", { text, type: t(`scan.types.${b ? typeOf(b) : "u8"}`) });
};
</script>

<template>
  <div>
    <p class="muted help">{{ t("bookmarks.help") }}</p>
    <div v-if="bookmarks.list.length" class="gh">
      <div class="gh-head cols" aria-hidden="true"><span>{{ t("bookmarks.name") }}</span><span>{{ t("bookmarks.address") }}</span><span>{{ t("bookmarks.type") }}</span><span>{{ t("bookmarks.value") }}</span><span /></div>
      <div class="gh-rows">
        <div v-for="b in bookmarks.list" :key="b.id" class="gh-row cols">
          <input class="plain" :value="b.name" :aria-label="t('bookmarks.name')" data-keep-keys @change="(e: Event) => renameBookmark(b.id, (e.target as HTMLInputElement).value)" />
          <AddressLink :address="b.address" :length="b.length" />
          <select :value="typeOf(b)" :aria-label="t('bookmarks.type')" @change="(e: Event) => setBookmarkType(b.id, (e.target as HTMLSelectElement).value as ScanType)">
            <option v-for="k in SCAN_TYPES" :key="k" :value="k">{{ t(`scan.shortTypes.${k}`) }}</option>
          </select>
          <ValueField :value="values[b.id] ?? null" :type="typeOf(b)" :label="t('bookmarks.value')" @commit="(v) => write(b.id, v)" @invalid="(text) => invalid(b.id, text)" />
          <button class="flat icon" :title="t('bookmarks.remove')" :aria-label="t('bookmarks.remove')" @click="removeBookmark(b.id)"><Icon name="trash" :size="14" /></button>
        </div>
      </div>
    </div>
    <p v-else class="muted">{{ state.loaded ? t("bookmarks.none") : t("bookmarks.noApp") }}</p>
    <p v-if="message" class="error" role="alert">{{ message }}</p>
  </div>
</template>

<style scoped>
.help { margin: 0 0 8px; font-size: 12px; }
.cols { grid-template-columns: minmax(70px, 1.1fr) minmax(100px, 1.1fr) 62px minmax(70px, 1.2fr) 28px; }
.plain { min-width: 0; width: 100%; height: 22px; padding: 0 5px; border-color: transparent; background: transparent; }
.plain:hover, .plain:focus { border-color: var(--line); background: var(--surface); }
select { min-width: 0; height: 22px; }
.gh-rows { max-height: 220px; overflow: auto; }
</style>
