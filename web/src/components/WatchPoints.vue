<script setup lang="ts">
// Points d'observation : on choisit une plage de mémoire, et l'on voit quelles instructions de l'application la lisent ou
// l'écrivent. Avec « interrompre », l'application s'arrête juste après le premier accès.
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { showInMemory } from "../ui.js";
import { hex0x, parseAddress, valueOf, checkedOf } from "../util.js";
import { poll } from "../poll.js";
import { addWatch, clearHits, drainHits, refreshWatches, removeWatch, watching } from "../watchStore.js";
import { setStop } from "../pointsStore.js";
import type { WatchKind } from "../watchStore.js";
import Icon from "./Icon.vue";
import AddressLink from "./AddressLink.vue";

const { t } = useI18n();
const emulator = useEmu();
const { state } = emulator;

const address = ref("");
const length = ref("4");
const kind = ref<WatchKind>("write");
const stop = ref(false);
const error = ref("");

async function add() {
  const start = parseAddress(address.value);
  const size = Number.parseInt(length.value, 10);
  if (start === undefined || !(size > 0)) return void (error.value = t("watch.invalid"));
  error.value = (await addWatch(emulator, start, size, kind.value, stop.value)) ?? "";
  if (!error.value) address.value = "";
}

let stopPolling = () => {};
onMounted(() => {
  stopPolling = poll(async () => {
    if (!state.ready) return;
    await refreshWatches(emulator);
    if (watching.list.length > 0) await drainHits(emulator);
  }, 300);
});
onBeforeUnmount(() => stopPolling());

const KIND_LETTER: Record<string, string> = { read: "L", write: "É" };
</script>

<template>
  <div class="watch">
    <h3 class="subhead">{{ t("watch.new") }}</h3>
    <form class="form" @submit.prevent="add">
      <label>{{ t("watch.address") }}<input :value="address" class="mono" placeholder="0x20020000" data-keep-keys @input="address = valueOf($event)" /></label>
      <label>{{ t("watch.length") }}<input :value="length" class="mono short" data-keep-keys @input="length = valueOf($event)" /></label>
      <label>{{ t("watch.kind") }}
        <select :value="kind" @change="kind = valueOf($event) as WatchKind">
          <option value="write">{{ t("watch.kinds.write") }}</option>
          <option value="read">{{ t("watch.kinds.read") }}</option>
          <option value="readwrite">{{ t("watch.kinds.readwrite") }}</option>
        </select>
      </label>
      <label class="check" :title="t('watch.stopHint')"><input type="checkbox" :checked="stop" @change="stop = checkedOf($event)" />{{ t("watch.stop") }}</label>
      <button type="submit" :disabled="!state.loaded">{{ t("watch.add") }}</button>
    </form>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <p v-if="state.stop?.kind === 'watchpoint'" class="stopped" role="status">
      {{ t("watch.stopped", { pc: hex0x(Number(state.stop.pc)), address: hex0x(Number(state.stop.address)) }) }}
    </p>

    <h3 class="subhead">{{ t("watch.posed") }}</h3>
    <ul v-if="watching.list.length" class="list">
      <li class="cols head" aria-hidden="true">
        <span>{{ t("points.colWhere") }}</span><span>{{ t("points.colSize") }}</span><span>{{ t("watch.kind") }}</span>
        <span class="center" :title="t('points.colStopHint')">{{ t("points.colStop") }}</span><span />
      </li>
      <li v-for="w in watching.list" :key="w.id" class="cols">
        <AddressLink :address="w.address" :length="w.length" />
        <span class="mono">{{ w.length }}</span>
        <span>{{ t(`watch.kinds.${w.kind}`) }}</span>
        <span class="center"><input type="checkbox" :checked="w.stop" :title="t('watch.stopHint')" :aria-label="t('watch.stop')" @change="setStop(emulator, w, ($event.target as HTMLInputElement).checked)" /></span>
        <button class="flat icon" :title="t('watch.remove')" :aria-label="t('watch.remove')" @click="removeWatch(emulator, w.id)"><Icon name="trash" :size="14" /></button>
      </li>
    </ul>
    <p v-else class="muted">{{ t("watch.none") }}</p>

    <h3 class="subhead" v-if="watching.list.length">{{ t("watch.accesses") }}</h3>
    <div v-if="watching.list.length" class="gh">
      <div class="gh-head cols" aria-hidden="true"><span>{{ t("watch.colInstruction") }}</span><span>{{ t("watch.colAccess") }}</span><span>{{ t("watch.colValue") }}</span><span>{{ t("watch.colCount") }}</span></div>
      <div class="gh-rows">
        <div v-for="row in watching.rows" :key="row.key" class="gh-row cols">
          <AddressLink :address="row.pc" view="code" />
          <span class="mono">{{ KIND_LETTER[row.write ? "write" : "read"] }}{{ row.width * 8 }} {{ hex0x(row.address) }}</span>
          <span class="mono">{{ hex0x(row.value, row.width * 2) }}</span>
          <span class="mono">{{ row.count }}</span>
        </div>
        <p v-if="!watching.rows.length" class="empty muted">{{ t("watch.waiting") }}</p>
      </div>
      <div class="gh-status"><button @click="clearHits">{{ t("watch.clear") }}</button></div>
    </div>
  </div>
</template>

<style scoped>
.watch { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; }
.form { padding: 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface-2); display: flex; flex-wrap: wrap; gap: 8px; align-items: flex-end; }
.form label { display: grid; gap: 2px; font-size: 12px; color: var(--muted); }
.form label.check { display: flex; align-items: center; gap: 4px; color: inherit; }
.form input.mono { width: 9.5em; }
.form input.short { width: 4.5em; }
.list { list-style: none; margin: 0; padding: 0; display: grid; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
.list li.cols { display: grid; grid-template-columns: minmax(9em, 1.6fr) 5em minmax(5em, 1.2fr) 8em 2em; gap: 12px; align-items: center; padding: 4px 10px; }
.list li.cols:nth-child(even) { background: var(--surface-2); }
.list li.head { white-space: nowrap; background: var(--surface-2); border-bottom: 1px solid var(--line); font-size: 11px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--muted); }
.center { text-align: center; justify-self: center; }
.stopped { margin: 0; padding: 6px 8px; border-radius: 6px; background: var(--accent-soft); font-size: 12.5px; }
.cols { grid-template-columns: 100px 1.3fr 1fr 60px; }
.link { border: none; background: none; padding: 0; text-align: left; color: var(--accent-strong); cursor: pointer; }
.link:hover { text-decoration: underline; }
.empty { padding: 14px; text-align: center; margin: 0; }
.gh-rows { max-height: 260px; overflow: auto; }
</style>
