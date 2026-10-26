<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

const props = defineProps({
  /** Éléments : { label, action, danger?, disabled?, separator? } */
  items: { type: Array, required: true },
  x: { type: Number, required: true },
  y: { type: Number, required: true },
});
const emit = defineEmits(["close"]);
const menu = ref(null);
const pos = ref({ left: props.x, top: props.y });

onMounted(async () => {
  await nextTick();
  const box = menu.value.getBoundingClientRect();
  pos.value = { left: Math.min(props.x, innerWidth - box.width - 6), top: Math.min(props.y, innerHeight - box.height - 6) };
  menu.value.querySelector("button:not(:disabled)")?.focus();
  addEventListener("pointerdown", outside, true);
  addEventListener("blur", () => emit("close"));
});
onBeforeUnmount(() => removeEventListener("pointerdown", outside, true));
watch(() => [props.x, props.y], () => (pos.value = { left: props.x, top: props.y }));

function outside(event) {
  if (!menu.value?.contains(event.target)) emit("close");
}
function key(event) {
  const buttons = [...menu.value.querySelectorAll("button:not(:disabled)")];
  const i = buttons.indexOf(document.activeElement);
  if (event.key === "Escape") emit("close");
  else if (event.key === "ArrowDown") buttons[(i + 1) % buttons.length]?.focus();
  else if (event.key === "ArrowUp") buttons[(i - 1 + buttons.length) % buttons.length]?.focus();
  else return;
  event.preventDefault();
  event.stopPropagation();
}
function run(item) {
  emit("close");
  item.action?.();
}
</script>

<template>
  <Teleport to="body">
    <ul ref="menu" class="menu" role="menu" :style="{ left: pos.left + 'px', top: pos.top + 'px' }" @keydown="key" @contextmenu.prevent>
      <template v-for="(item, i) in items" :key="i">
        <li v-if="item.separator" role="separator" class="separator" />
        <li v-else role="none">
          <button role="menuitem" :class="{ danger: item.danger }" :disabled="item.disabled" @click="run(item)">{{ item.label }}</button>
        </li>
      </template>
    </ul>
  </Teleport>
</template>

<style scoped>
.menu {
  position: fixed; z-index: 100; list-style: none; margin: 0; padding: 4px; min-width: 170px; background: var(--surface);
  border: 1px solid var(--line); border-radius: 8px; box-shadow: var(--shadow);
}
.menu button { width: 100%; border: none; background: transparent; justify-content: flex-start; border-radius: 5px; }
.menu button:hover:not(:disabled), .menu button:focus-visible { background: var(--accent-soft); }
.separator { height: 1px; margin: 4px 2px; background: var(--line); }
</style>
