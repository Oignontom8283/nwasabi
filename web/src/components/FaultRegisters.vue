<script setup lang="ts">
// Les registres de faute du processeur simulé (CFSR, HFSR, DFSR, MMFAR, BFAR) : ce qu'un débogueur lirait après un plantage sur
// une vraie carte. Seuls les bits levés sont listés, avec leur explication.
import { onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { CFSR_BITS, DFSR_BITS, HFSR_BITS, SCB, hasFault, setBits } from "../faultRegs.js";
import type { FaultRegisters } from "../faultRegs.js";
import { showInMemory } from "../ui.js";
import { assignIfChanged, hex, hex0x, hexToBytes } from "../util.js";
import AddressLink from "./AddressLink.vue";

const { t } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;
const regs = ref<FaultRegisters | null>(null);

async function word(address: number): Promise<number> {
  const reply = await quiet({ cmd: "memory_read", address, length: 4 });
  if (!reply.ok) return 0;
  const b = hexToBytes(String(reply.hex));
  return ((b[0] ?? 0) | ((b[1] ?? 0) << 8) | ((b[2] ?? 0) << 16) | ((b[3] ?? 0) << 24)) >>> 0;
}

async function refresh() {
  if (!state.loaded) return void (regs.value = null);
  assignIfChanged(regs, {
    cfsr: await word(SCB.CFSR),
    hfsr: await word(SCB.HFSR),
    dfsr: await word(SCB.DFSR),
    mmfar: await word(SCB.MMFAR),
    bfar: await word(SCB.BFAR),
  });
}
onMounted(refresh);
// Une faute arrête l'application : on relit à chaque arrêt.
watch(() => [state.stop, state.revision], refresh);

const rows = (): Array<{ reg: string; value: number; bits: ReturnType<typeof setBits> }> => {
  const r = regs.value;
  if (!r) return [];
  return [
    { reg: "CFSR", value: r.cfsr, bits: setBits(r.cfsr, CFSR_BITS) },
    { reg: "HFSR", value: r.hfsr, bits: setBits(r.hfsr, HFSR_BITS) },
    { reg: "DFSR", value: r.dfsr, bits: setBits(r.dfsr, DFSR_BITS) },
  ].filter((row) => row.value !== 0);
};
</script>

<template>
  <div class="faultregs">
    <template v-if="regs && hasFault(regs)">
      <table class="mono">
        <tbody>
          <template v-for="row in rows()" :key="row.reg">
            <tr class="head"><td>{{ row.reg }}</td><td>{{ hex0x(row.value) }}</td><td /></tr>
            <tr v-for="b in row.bits" :key="b.name" class="bit">
              <td class="muted">bit {{ b.bit }}</td><td>{{ b.name }}</td><td class="why">{{ t(`faultregs.bits.${b.key}`) }}</td>
            </tr>
          </template>
          <tr v-if="regs.bfar && (regs.cfsr & 0x8000)" class="head">
            <td>BFAR</td><td><AddressLink :address="regs.bfar" /></td><td class="why">{{ t("faultregs.bfar") }}</td>
          </tr>
          <tr v-if="regs.cfsr & 0x80" class="head">
            <td>MMFAR</td><td><AddressLink :address="regs.mmfar" /></td><td class="why">{{ t("faultregs.mmfar") }}</td>
          </tr>
        </tbody>
      </table>
    </template>
    <p v-else class="muted">{{ t("faultregs.none") }}</p>
  </div>
</template>

<style scoped>
table { border-spacing: 0; width: 100%; font-size: 12.5px; }
td { padding: 2px 8px 2px 0; vertical-align: top; }
tr.head td { padding-top: 6px; font-weight: 600; }
tr.bit td:first-child { padding-left: 10px; }
.why { font-family: system-ui, sans-serif; color: var(--muted); }
.link { border: none; background: none; padding: 0; font: inherit; color: var(--accent-strong); cursor: pointer; text-decoration: underline dotted; }
p { margin: 0; font-size: 12px; }
</style>
