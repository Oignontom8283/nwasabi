<script setup lang="ts">
// Une adresse (ou une plage d'adresses) sur laquelle on peut cliquer. Le clic montre la mémoire ; le clic droit propose le reste :
// la voir dans l'éditeur hexadécimal ou dans le code désassemblé, la copier, y poser un point d'arrêt, l'observer, la mettre en
// signet. Une plage (plus d'un octet) est en orange et s'affiche présélectionnée.
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { addBookmark } from "../bookmarks.js";
import { addBreak } from "../pointsStore.js";
import { notify, showBreakpoints, showInDebugger, showInMemory, showWatchpoints } from "../ui.js";
import { hex0x } from "../util.js";
import { addWatch } from "../watchStore.js";
import ContextMenu from "./ContextMenu.vue";
import type { MenuItem } from "./ContextMenu.vue";

const props = withDefaults(
  defineProps<{
    address: number;
    /** Nombre d'octets : au-delà de 1, c'est une plage. */
    length?: number;
    /** Texte à la place de l'adresse (un nom de fonction, par exemple). */
    label?: string;
    /** Ce que fait un clic simple : l'éditeur hexadécimal, ou le code désassemblé. */
    view?: "memory" | "code";
  }>(),
  { length: 1, label: "", view: "memory" },
);

const { t } = useI18n();
const emulator = useEmu();
const isRange = computed(() => props.length > 1);
const text = computed(() => props.label || hex0x(props.address));

const open = (view: "memory" | "code") => (view === "code" ? showInDebugger(props.address, props.length) : showInMemory(props.address, props.length));

const menu = ref<{ x: number; y: number; items: MenuItem[] } | null>(null);
async function copy(value: string) {
  try {
    await navigator.clipboard.writeText(value);
    notify(t("debug.menu.copied"));
  } catch {
    notify(t("debug.menu.copyFailed"));
  }
}
async function watch() {
  const failure = await addWatch(emulator, props.address, props.length, "write", false);
  if (failure) notify(failure);
  else showWatchpoints();
}
/** Touche « menu contextuel » ou Maj + F10 : ouvre le menu à côté du lien, comme un clic droit. */
function menuKey(event: KeyboardEvent) {
  if (event.key !== "ContextMenu" && !(event.key === "F10" && event.shiftKey)) return;
  event.preventDefault();
  const box = (event.currentTarget as Element).getBoundingClientRect();
  openMenu({ clientX: box.left, clientY: box.bottom });
}
function openMenu(event: { clientX: number; clientY: number }) {
  menu.value = {
    x: event.clientX,
    y: event.clientY,
    items: [
      { label: t("link.inMemory"), action: () => open("memory") },
      { label: t("link.inCode"), action: () => open("code") },
      { separator: true },
      { label: isRange.value ? t("link.copyRange", { range: `${hex0x(props.address)}–${hex0x(props.address + props.length - 1)}` }) : t("debug.menu.copyAddress"), action: () => void copy(isRange.value ? `${hex0x(props.address)}-${hex0x(props.address + props.length - 1)}` : hex0x(props.address, 8)) },
      { label: t("link.break"), action: () => void addBreak(emulator, props.address & ~1).then(() => showBreakpoints()) },
      { label: t("link.watch"), action: () => void watch() },
      { label: t("link.bookmark"), action: () => notify(addBookmark(props.address, props.length) ? t("bookmarks.added", { address: hex0x(props.address) }) : t("bookmarks.noApp")) },
    ],
  };
}
</script>

<template>
  <button
    class="address-link mono"
    :class="{ range: isRange }"
    :title="`${isRange ? t('link.rangeHint', { n: length }, length) + '\n' : ''}${t(view === 'code' ? 'link.hintCode' : 'link.hintMemory')}`"
    aria-haspopup="menu"
    @click="open(view)"
    @contextmenu.prevent.stop="openMenu"
    @keydown="menuKey"
  >{{ text }}<span v-if="isRange" class="size">+{{ length }}</span></button>
  <ContextMenu v-if="menu" :items="menu.items" :x="menu.x" :y="menu.y" @close="menu = null" />
</template>

<style scoped>
.address-link { border: none; background: none; padding: 0; font: inherit; text-align: left; color: var(--accent-strong); cursor: pointer; }
.address-link:hover { text-decoration: underline; }
/* Une plage se distingue des adresses seules : orange, avec sa taille. */
.address-link.range { color: var(--warn); }
.size { margin-left: 4px; font-size: 0.85em; padding: 0 4px; border: 1px solid currentColor; border-radius: 6px; opacity: 0.85; }
</style>
