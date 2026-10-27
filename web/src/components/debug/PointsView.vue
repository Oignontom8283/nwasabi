<script setup lang="ts">
// Tous les points d'arrêt et d'observation au même endroit : on les pose, on les désactive sans les perdre, on les retire.
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../../emulator.js";
import { addBreak, describeCondition, disable, enable, points, refreshPoints, removeBreak, removeWatchPoint, setStop } from "../../pointsStore.js";
import type { Condition } from "../../pointsStore.js";
import { ui, showInDebugger, showInMemory } from "../../ui.js";
import { hex0x, parseAddress, poll } from "../../util.js";
import Icon from "../Icon.vue";
import AddressLink from "../AddressLink.vue";

const { t } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;

const where = ref("");
const useCondition = ref(false);
const register = ref("r0");
const op = ref("==");
const value = ref("0");
const error = ref("");
const names = ref<Record<number, string>>({});

// Une adresse demandée depuis le code (clic droit « point d'arrêt conditionnel ») remplit le formulaire.
watch(
  () => points.pending,
  (address) => {
    if (address === null) return;
    where.value = hex0x(address);
    useCondition.value = true;
    points.pending = null;
  },
  { immediate: true },
);

let stopPolling = () => {};
onMounted(() => {
  stopPolling = poll(async () => {
    if (!state.loaded) return;
    await refreshPoints(emulator);
    for (const b of points.breaks) {
      if (names.value[b.address] !== undefined) continue;
      const sym = await quiet({ cmd: "symbol", address: b.address });
      names.value[b.address] = sym.ok && sym.name ? `${sym.name}${Number(sym.offset) ? `+${sym.offset}` : ""}` : "";
    }
  }, 500);
});
onBeforeUnmount(() => stopPolling());

const REGISTERS = ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "sp", "lr", "pc"];
const OPS = ["==", "!=", "<", "<=", ">", ">=", "&"];

async function add() {
  error.value = "";
  let address = parseAddress(where.value);
  if (address === undefined) {
    const found = (await quiet({ cmd: "symbols", query: where.value.trim() })).symbols as Array<{ name: string; address: number }> | undefined;
    address = (found?.find((s) => s.name === where.value.trim()) ?? found?.[0])?.address;
  }
  if (address === undefined) return void (error.value = t("points.badAddress"));
  let condition: Condition | undefined;
  if (useCondition.value) {
    const n = parseAddress(value.value);
    if (n === undefined) return void (error.value = t("points.badValue"));
    condition = { register: register.value, op: op.value, value: n };
  }
  error.value = (await addBreak(emulator, address, condition)) ?? "";
  if (!error.value) where.value = "";
}

const empty = computed(() => !points.breaks.length && !points.watches.length && !points.disabled.length);
const kindText = (kind: string) => t(`watch.kinds.${kind}`);
</script>

<template>
  <div class="points">
    <p v-if="!state.loaded" class="empty muted">{{ t("debug.noApp") }}</p>
    <template v-else>
      <h3>{{ t("points.add") }}</h3>
      <form class="form" @submit.prevent="add">
        <label class="grow">{{ t("points.where") }}<input v-model="where" class="mono" :placeholder="t('points.wherePlaceholder')" data-keep-keys /></label>
        <label class="check"><input v-model="useCondition" type="checkbox" /> {{ t("points.onlyIf") }}</label>
        <template v-if="useCondition">
          <select v-model="register" :aria-label="t('points.register')"><option v-for="r in REGISTERS" :key="r" :value="r">{{ r }}</option></select>
          <select v-model="op" :aria-label="t('points.compare')"><option v-for="o in OPS" :key="o" :value="o">{{ o }}</option></select>
          <input v-model="value" class="mono short" :aria-label="t('points.value')" data-keep-keys />
        </template>
        <button type="submit" class="primary">{{ t("points.addBreak") }}</button>
      </form>
      <p class="muted small">{{ t("points.conditionHelp") }}</p>
      <p v-if="error" class="error" role="alert">{{ error }}</p>

      <h3>{{ t("points.list") }}</h3>
      <p v-if="empty" class="muted small">{{ t("points.none") }}</p>
      <ul v-else class="list">
        <li class="cols head" aria-hidden="true">
          <span>{{ t("points.colType") }}</span><span>{{ t("points.colWhere") }}</span><span>{{ t("points.colSize") }}</span><span>{{ t("points.colDetail") }}</span>
          <span class="center" :title="t('points.colStopHint')">{{ t("points.colStop") }}</span><span class="center">{{ t("points.colActions") }}</span>
        </li>
        <li v-for="b in points.breaks" :key="`b${b.address}`" class="cols">
          <span class="kind">{{ t("points.break") }}</span>
          <span class="where"><AddressLink :address="b.address" view="code" /> <span class="muted mono name">{{ names[b.address] }}</span></span>
          <span class="mono muted">—</span>
          <span>
            <span v-if="b.condition" class="chip" :title="t('points.condition')">{{ t("points.if") }} {{ describeCondition(b.condition) }}</span>
            <span v-else class="muted">—</span>
          </span>
          <span class="center"><input type="checkbox" checked disabled :title="t('points.alwaysStops')" :aria-label="t('watch.stop')" /></span>
          <span class="actions">
            <button class="flat" @click="disable(emulator, { kind: 'break', point: b })">{{ t("points.disable") }}</button>
            <button class="flat icon" :title="t('debug.removeBreak')" :aria-label="t('debug.removeBreak')" @click="removeBreak(emulator, b.address)"><Icon name="trash" :size="14" /></button>
          </span>
        </li>
        <li v-for="w in points.watches" :key="`w${w.id}`" class="cols">
          <span class="kind">{{ t("points.watch") }}</span>
          <span class="where"><AddressLink :address="w.address" :length="w.length" /></span>
          <span class="mono">{{ w.length }}</span>
          <span class="muted">{{ kindText(w.kind) }}</span>
          <span class="center"><input type="checkbox" :checked="w.stop" :title="t('watch.stopHint')" :aria-label="t('watch.stop')" @change="setStop(emulator, w, ($event.target as HTMLInputElement).checked)" /></span>
          <span class="actions">
            <button class="flat" @click="disable(emulator, { kind: 'watch', point: { address: w.address, length: w.length, kind: w.kind, stop: w.stop } })">{{ t("points.disable") }}</button>
            <button class="flat icon" :title="t('watch.remove')" :aria-label="t('watch.remove')" @click="removeWatchPoint(emulator, w.id)"><Icon name="trash" :size="14" /></button>
          </span>
        </li>
        <li v-for="(d, i) in points.disabled" :key="`d${i}`" class="cols off">
          <span class="kind">{{ d.kind === "break" ? t("points.break") : t("points.watch") }}</span>
          <span class="where"><AddressLink :address="d.point.address" :length="d.kind === 'watch' ? d.point.length : 1" :view="d.kind === 'break' ? 'code' : 'memory'" /></span>
          <span class="mono">{{ d.kind === "watch" ? d.point.length : "—" }}</span>
          <span><span class="chip">{{ t("points.disabled") }}</span></span>
          <span class="center"><input type="checkbox" :checked="d.kind === 'break' || d.point.stop" disabled :aria-label="t('watch.stop')" /></span>
          <span class="actions">
            <button class="flat" @click="enable(emulator, i)">{{ t("points.enable") }}</button>
            <button class="flat icon" :title="t('points.forget')" :aria-label="t('points.forget')" @click="points.disabled.splice(i, 1)"><Icon name="trash" :size="14" /></button>
          </span>
        </li>
      </ul>
      <p class="muted small"><button class="link" @click="ui.debugView = 'watch'">{{ t("points.toWatch") }}</button></p>
    </template>
  </div>
</template>

<style scoped>
.points { display: grid; gap: 8px; align-content: start; }
.empty { padding: 24px 8px; text-align: center; margin: 0; }
h3 { margin: 6px 0 0; font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
.small { margin: 0; font-size: 12px; }
.form { padding: 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface-2); display: flex; flex-wrap: wrap; gap: 8px; align-items: flex-end; }
.form label { display: grid; gap: 2px; font-size: 12px; color: var(--muted); }
.form label.grow { flex: 1 1 14em; }
.form label.check { display: flex; align-items: center; gap: 4px; color: inherit; }
.form input.short { width: 6em; }
.list { list-style: none; margin: 0; padding: 0; display: grid; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
.list li.cols { display: grid; grid-template-columns: 6em minmax(9em, 1.6fr) 5em minmax(5em, 1.2fr) 8em auto; gap: 12px; align-items: center; padding: 4px 10px; }
.list li.cols:nth-child(even) { background: var(--surface-2); }
.list li.head { white-space: nowrap; background: var(--surface-2); border-bottom: 1px solid var(--line); font-size: 11px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--muted); }
.list li.off { opacity: 0.6; }
.center { text-align: center; justify-self: center; }
.where { display: flex; gap: 8px; align-items: baseline; flex-wrap: wrap; min-width: 0; }
.kind { font-size: 12px; color: var(--muted); }
.name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.chip { font-size: 11px; padding: 0 6px; border: 1px solid var(--line); border-radius: 8px; white-space: nowrap; }
.actions { display: flex; gap: 4px; justify-content: flex-end; }
.link { border: none; background: none; padding: 0; color: var(--accent-strong); cursor: pointer; font: inherit; }
.link:hover { text-decoration: underline; }
</style>
