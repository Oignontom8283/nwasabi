<script setup>
import { inject, onMounted, ref } from "vue";

const { state, attach } = inject("emulator");
const canvas = ref(null);
onMounted(() => attach(canvas.value));

function describe(stop) {
  if (!stop) return "—";
  switch (stop.kind) {
    case "exited": return `terminée (code ${stop.code})`;
    case "fault": return `faute : ${stop.fault}${stop.address !== undefined ? ` à 0x${stop.address.toString(16)}` : ""}`;
    case "home_pressed": return "quittée (touche Home)";
    case "wait_event": return "attend une touche";
    case "watchpoint": return `point d'observation à 0x${stop.address.toString(16)}`;
    default: return state.running ? "en cours" : stop.kind;
  }
}
</script>

<template>
  <div class="screen">
    <canvas ref="canvas" width="320" height="240" />
    <p class="status mono" :class="{ error: state.stop?.kind === 'fault' }">
      {{ describe(state.stop) }} · {{ state.frames }} images
    </p>
    <p v-if="state.error" class="error">{{ state.error }}</p>
  </div>
</template>

<style scoped>
.screen { width: 640px; max-width: 100%; }
canvas {
  width: 100%;
  aspect-ratio: 4 / 3;
  image-rendering: pixelated;
  background: #000;
  border: 2px solid var(--line);
  border-radius: 6px;
  display: block;
}
.status { margin: 6px 0 0; color: var(--muted); }
</style>
