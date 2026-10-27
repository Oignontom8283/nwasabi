<script setup lang="ts">
// La liste des adresses suivies : valeur relue en continu, modifiable, ou maintenue.
import { onBeforeUnmount, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { listMessage, memoryAccess, store } from "../scanStore.js";
import type { Watched } from "../scanStore.js";
import { hex0x } from "../util.js";
import { poll } from "../poll.js";
import AddressLink from "./AddressLink.vue";
import Icon from "./Icon.vue";
import ValueField from "./ValueField.vue";

const { t } = useI18n();
const emulator = useEmu();
const { readValue, writeValue } = memoryAccess(emulator);

const remove = (id: number) => (store.watched = store.watched.filter((w) => w.id !== id));
const hold = (entry: Watched, on: boolean) => (entry.frozen = on ? entry.current : null);

async function write(entry: Watched, value: number): Promise<boolean> {
  const ok = await writeValue(entry.address, entry.type, value);
  if (!ok) listMessage.value = t("scan.writeFailed", { address: hex0x(entry.address) });
  return ok;
}

const invalid = (entry: Watched, text: string) => (listMessage.value = t("scan.badValue", { text, type: t(`scan.types.${entry.type}`) }));

/** La valeur saisie (voir `ValueField`) est écrite en mémoire, dans le format du type de l'adresse. */
async function commit(entry: Watched, value: number) {
  listMessage.value = "";
  if (await write(entry, value)) {
    entry.current = value;
    if (entry.frozen !== null) entry.frozen = value;
  }
}

let stopPolling = () => {};
onMounted(() => {
  stopPolling = poll(async () => {
    if (!emulator.state.loaded) return;
    // Les valeurs sont relues ; une valeur maintenue est réécrite dès qu'elle change.
    for (const entry of store.watched) {
      const value = await readValue(entry.address, entry.type);
      if (value !== entry.current) entry.current = value;
      if (entry.frozen !== null && value !== null && value !== entry.frozen && (await write(entry, entry.frozen))) entry.current = entry.frozen;
    }
  }, 250);
});
onBeforeUnmount(() => stopPolling());
</script>

<template>
  <div>
    <div class="gh">
      <div class="gh-head wcols" aria-hidden="true"><span>{{ t("scan.colDescription") }}</span><span>{{ t("scan.colAddress") }}</span><span>{{ t("scan.colType") }}</span><span>{{ t("scan.colValue") }}</span><span>{{ t("scan.colFrozen") }}</span><span /></div>
      <div class="gh-rows">
        <div v-for="w in store.watched" :key="w.id" class="gh-row wcols">
          <input v-model="w.description" class="plain" :placeholder="t('scan.unnamed')" :aria-label="t('scan.colDescription')" data-keep-keys />
          <AddressLink :address="w.address" />
          <span class="muted">{{ t(`scan.shortTypes.${w.type}`) }}</span>
          <ValueField :value="w.frozen ?? w.current" :type="w.type" :label="t('scan.colValue')" @commit="(v) => commit(w, v)" @invalid="(text) => invalid(w, text)" />
          <input type="checkbox" :checked="w.frozen !== null" :disabled="w.current === null" :aria-label="t('scan.colFrozen')" @change="(e: Event) => hold(w, (e.target as HTMLInputElement).checked)" />
          <button class="flat icon" :title="t('scan.remove')" :aria-label="t('scan.remove')" @click="remove(w.id)"><Icon name="trash" :size="14" /></button>
        </div>
        <p v-if="!store.watched.length" class="empty muted">{{ t("scan.listEmpty") }}</p>
      </div>
    </div>
    <p v-if="listMessage" class="error" role="alert">{{ listMessage }}</p>
  </div>
</template>

<style scoped>
.wcols { grid-template-columns: minmax(70px, 1.2fr) 100px 52px minmax(70px, 1fr) 42px 28px; }
.link { border: none; background: none; padding: 0; text-align: left; color: var(--accent-strong); cursor: pointer; }
.link:hover { text-decoration: underline; }
.plain { min-width: 0; width: 100%; height: 22px; padding: 0 5px; }
.plain { border-color: transparent; background: transparent; }
.plain:hover, .plain:focus { border-color: var(--line); background: var(--surface); }
.empty { padding: 14px; text-align: center; margin: 0; }
.gh-rows { max-height: 260px; overflow: auto; }
</style>
