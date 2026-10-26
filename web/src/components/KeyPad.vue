<script setup>
import { inject } from "vue";
import { KEY_ROWS, PAD } from "../keys.js";

const { key } = inject("emulator");

// Un appui souris ou tactile reste enfoncé tant que le bouton est maintenu.
function bind(name) {
  return {
    onPointerdown: (e) => { e.currentTarget.setPointerCapture(e.pointerId); key(name, true); },
    onPointerup: () => key(name, false),
    onPointercancel: () => key(name, false),
  };
}
</script>

<template>
  <div class="pad">
    <div class="dpad">
      <button class="k up" v-bind="bind('up')">{{ PAD.up }}</button>
      <button class="k left" v-bind="bind('left')">{{ PAD.left }}</button>
      <button class="k ok" v-bind="bind('ok')">{{ PAD.ok }}</button>
      <button class="k right" v-bind="bind('right')">{{ PAD.right }}</button>
      <button class="k down" v-bind="bind('down')">{{ PAD.down }}</button>
    </div>
    <div v-for="(row, i) in KEY_ROWS" :key="i" class="row">
      <button v-for="[name, label] in row" :key="name" class="k" v-bind="bind(name)">{{ label }}</button>
    </div>
    <p class="muted hint">Clavier et gestes sur l'écran : voir l'onglet « Touches ».</p>
  </div>
</template>

<style scoped>
.pad { width: 640px; max-width: 100%; user-select: none; touch-action: none; }
.row { display: flex; gap: 6px; margin-bottom: 6px; }
.k { flex: 1; min-width: 0; padding: 8px 2px; white-space: nowrap; }
.k:active { background: var(--accent); color: var(--accent-text); }
.dpad {
  display: grid;
  grid-template-columns: 64px 64px 64px;
  grid-template-areas: ". up ." "left ok right" ". down .";
  gap: 4px;
  margin: 0 auto 10px;
  width: max-content;
}
.up { grid-area: up; } .left { grid-area: left; } .ok { grid-area: ok; } .right { grid-area: right; } .down { grid-area: down; }
.hint { margin: 4px 0 0; font-size: 12px; }
</style>
