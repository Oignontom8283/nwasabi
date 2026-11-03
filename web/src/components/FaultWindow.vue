<script setup lang="ts">
// Fenêtre qui s'ouvre quand l'application s'arrête sur une faute : ce qui s'est passé en clair, l'instruction fautive, l'état des
// registres, et un bouton pour l'examiner dans le débogueur.
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { showInDebugger } from "../ui.js";
import { hex, hex0x } from "../util.js";
import AddressLink from "./AddressLink.vue";
import FaultRegisters from "./FaultRegisters.vue";
import FloatingWindow from "./FloatingWindow.vue";

const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;

interface Line {
  address: number;
  hex: string;
  mnemonic: string;
  operands: string;
}
const line = ref<Line | null>(null);
const where = ref<string | null>(null);
const regs = ref<Array<[string, number]>>([]);

const stop = computed(() => state.stop as Record<string, unknown> | null);
const kind = computed(() => String(stop.value?.fault ?? ""));
const num = (key: string) => Number(stop.value?.[key] ?? 0);

/** Adresse de l'instruction fautive : celle de la faute, sinon `pc` (laissé sur l'instruction par le processeur). */
const pc = ref(0);

onMounted(async () => {
  const cpu = await quiet({ cmd: "cpu" });
  if (!cpu.ok) return;
  pc.value = stop.value?.pc !== undefined ? num("pc") : (cpu.pc as number);
  const r = cpu.r as number[];
  regs.value = [...[0, 1, 2, 3].map((i): [string, number] => [`r${i}`, r[i] as number]), ["sp", cpu.sp as number], ["lr", cpu.lr as number], ["pc", cpu.pc as number]];
  const code = await quiet({ cmd: "disasm", address: pc.value, count: 1 });
  line.value = (code.ok ? (code.lines as Line[])[0] : null) ?? null;
  const sym = await quiet({ cmd: "symbol", address: pc.value });
  where.value = sym.ok && sym.name ? `${sym.name}${Number(sym.offset) ? `+${hex0x(Number(sym.offset), 1)}` : ""}` : null;
});

const title = computed(() => t(`fault.kinds.${kind.value}`, t("fault.kinds.other")));
const detail = computed(() => {
  switch (kind.value) {
    case "bus": {
      const access = t(String(stop.value?.write) === "true" ? "fault.write" : "fault.read");
      return t(`fault.bus.${String(stop.value?.access)}`, { access, size: num("width"), address: hex0x(num("address")) });
    }
    case "unimplemented":
      return t("fault.unimplemented", { word: hex(num("halfword"), 4) });
    case "usage":
      return t(`fault.usage.${String(stop.value?.kind)}`, t("fault.usage.other"));
    case "breakpoint":
      return t("fault.breakpoint", { imm: num("imm8") });
    case "unknown_svc":
      return t("fault.unknownSvc", { number: num("number") });
    default:
      return t("fault.waitForInterrupt");
  }
});

function debug() {
  showInDebugger(pc.value);
  emit("close");
}
async function restart() {
  emit("close");
  await emulator.restart();
}
</script>

<template>
  <FloatingWindow :width="500" :title="t('fault.title')" :resizable="false" @close="emit('close')">
    <div class="fault">
      <p class="what"><strong>{{ title }}</strong></p>
      <p class="detail">{{ detail }}</p>
      <section v-if="line" class="instr">
        <h3 class="subhead">{{ t("fault.instruction") }}</h3>
        <p class="mono code">
          <AddressLink :address="line.address" view="code" /> <span class="muted">{{ line.hex }}</span>
          <strong> {{ line.mnemonic }}</strong> {{ line.operands }}
        </p>
        <p v-if="where" class="muted small">{{ t("fault.in", { where }) }}</p>
      </section>
      <section v-if="regs.length">
        <h3 class="subhead">{{ t("fault.registers") }}</h3>
        <div class="regs mono">
          <span v-for="[name, value] in regs" :key="name"><span class="muted">{{ name }}</span> {{ hex(value) }}</span>
        </div>
      </section>
      <section>
        <h3 class="subhead">{{ t("fault.faultRegisters") }}</h3>
        <FaultRegisters />
      </section>
      <p class="muted small">{{ t("fault.note") }}</p>
      <div class="actions">
        <button class="primary" @click="debug">{{ t("fault.debug") }}</button>
        <button @click="restart">{{ t("fault.restart") }}</button>
        <button @click="emit('close')">{{ t("common.close") }}</button>
      </div>
    </div>
  </FloatingWindow>
</template>

<style scoped>
.fault { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; }
.fault p { margin: 0; }
.what { color: var(--danger); }
.code { padding: 6px 8px; border: 1px solid var(--line); border-radius: 6px; background: var(--surface-2); }
.addr { color: var(--accent-strong); }
.regs { display: grid; grid-template-columns: repeat(3, 1fr); gap: 2px 12px; font-size: 12.5px; }
.small { font-size: 12px; }
.actions { display: flex; flex-wrap: wrap; gap: 8px; justify-content: flex-end; }
</style>
