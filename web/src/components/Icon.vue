<script setup>
// Icônes en traits (24 × 24), dessinées pour ce projet : elles ne dépendent d'aucune police d'emojis.
defineProps({ name: { type: String, required: true }, size: { type: Number, default: 16 } });

const PATHS = {
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  paperclip: "M21 11l-8.5 8.5a5 5 0 0 1-7-7L14 4a3.5 3.5 0 0 1 5 5l-8.5 8.5a2 2 0 0 1-3-3L15 7",
  keyboard: "M3 6h18v12H3z M7 10h.01 M11 10h.01 M15 10h.01 M19 10h.01 M7 14h10",
  save: "M5 3h11l3 3v15H5z M8 3v6h7V3 M8 21v-7h8v7",
  pause: "M8 5v14 M16 5v14",
  play: "M7 4l13 8-13 8z",
  refresh: "M20 11a8 8 0 1 0-2.3 5.7 M20 4v7h-7",
  camera: "M4 8h3l2-3h6l2 3h3v11H4z M12 17a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
  maximize: "M4 9V4h5 M20 9V4h-5 M4 15v5h5 M20 15v5h-5",
  sliders: "M4 6h9 M17 6h3 M4 12h3 M11 12h9 M4 18h11 M19 18h1 M15 4v4 M9 10v4 M17 16v4",
  sun: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z M12 2v2 M12 20v2 M4.9 4.9l1.4 1.4 M17.7 17.7l1.4 1.4 M2 12h2 M20 12h2 M4.9 19.1l1.4-1.4 M17.7 6.3l1.4-1.4",
  moon: "M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z",
  contrast: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M12 3v18",
  menu: "M4 7h16 M4 12h16 M4 17h16",
  right: "M9 6l6 6-6 6",
  left: "M15 6l-6 6 6 6",
  close: "M6 6l12 12 M18 6L6 18",
  plus: "M12 5v14 M5 12h14",
  upload: "M12 16V4 M7 9l5-5 5 5 M4 20h16",
  download: "M12 4v12 M7 11l5 5 5-5 M4 20h16",
  trash: "M5 7h14 M10 11v6 M14 11v6 M6 7l1 13h10l1-13 M9 7V4h6v3",
  copy: "M9 9h11v11H9z M5 15V4h11",
  file: "M6 3h8l4 4v14H6z M14 3v4h4",
  chip: "M7 7h10v10H7z M10 10h4v4h-4z M9 3v4 M15 3v4 M9 17v4 M15 17v4 M3 9h4 M3 15h4 M17 9h4 M17 15h4",
  layers: "M12 3l9 5-9 5-9-5z M3 13l9 5 9-5",
  activity: "M3 12h4l3-8 4 16 3-8h4",
  power: "M12 3v9 M6.3 7a8 8 0 1 0 11.4 0",
  python: "M12 3c-4 0-4 1.5-4 3v2h4v1H6c-2 0-3 1.5-3 4s1 4 3 4h1v-2.5c0-2 1.5-3 3-3h4c1.5 0 2.5-1 2.5-2.5V6c0-1.5-1-3-4.5-3z M12 21c4 0 4-1.5 4-3v-2h-4v-1h6c2 0 3-1.5 3-4s-1-4-3-4h-1v2.5c0 2-1.5 3-3 3h-4c-1.5 0-2.5 1-2.5 2.5V18c0 1.5 1 3 4.5 3z",
  gear: "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
};
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" class="icon-svg">
    <path :d="PATHS[name]" :fill="name === 'play' || name === 'contrast' ? 'currentColor' : 'none'" :fill-opacity="name === 'contrast' ? 0.5 : 1" />
  </svg>
</template>

<style scoped>
.icon-svg { flex: none; vertical-align: -0.2em; }
</style>
