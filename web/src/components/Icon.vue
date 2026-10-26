<script setup lang="ts">
// Les icônes sont les fichiers SVG de `assets/icons/` (leurs origines et licences : `assets/README.md`). Celles en
// traits utilisent `currentColor` et prennent la couleur du texte ; les autres (disquette, GitHub) gardent la leur.
import { computed } from "vue";

const props = defineProps({ name: { type: String, required: true }, size: { type: Number, default: 16 } });

const FILES = import.meta.glob("../../../assets/icons/*.svg", { query: "?raw", import: "default", eager: true });
const ICONS = Object.fromEntries(Object.entries(FILES).map(([path, svg]) => [path.slice(path.lastIndexOf("/") + 1, -4), svg]));

const svg = computed(() => ICONS[props.name] ?? "");
</script>

<template>
  <span class="icon-svg" :style="{ width: size + 'px', height: size + 'px' }" aria-hidden="true" v-html="svg" />
</template>

<style scoped>
.icon-svg { display: inline-flex; flex: none; vertical-align: -0.2em; }
.icon-svg :deep(svg) { width: 100%; height: 100%; display: block; }
</style>
