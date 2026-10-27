<script setup lang="ts">
// Fenêtre flottante : se déplace par sa barre de titre, se redimensionne par le coin en bas à droite. Pendant un geste,
// la position et la taille sont écrites directement dans le style (sans repasser par Vue) et suivent le pointeur
// une fois par image : c'est ce qui évite les saccades. `modal` assombrit et bloque le reste de la page.
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import Icon from "./Icon.vue";

const props = withDefaults(defineProps<{ title?: string; width?: number; minWidth?: number; minHeight?: number; modal?: boolean; resizable?: boolean }>(), {
  title: "",
  width: 440,
  minWidth: 280,
  minHeight: 140,
  modal: false,
  resizable: true,
});
const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();

const win = ref<HTMLElement | null>(null);
let x = 0;
let y = 0;
let gesture: { kind: "move" | "size"; startX: number; startY: number; x: number; y: number; width: number; height: number } | null = null;
let frame = 0;
let pending: PointerEvent | null = null;

const place = () => {
  if (win.value) win.value.style.transform = `translate3d(${x}px, ${y}px, 0)`;
};

// Le focus revient à l'élément qui avait ouvert la fenêtre quand elle se ferme.
const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
onMounted(async () => {
  await nextTick();
  if (!win.value) return;
  const box = win.value.getBoundingClientRect();
  x = Math.max(8, (innerWidth - box.width) / 2);
  y = Math.max(8, (innerHeight - box.height) / 3);
  place();
  (win.value.querySelector<HTMLElement>("[data-autofocus]") ?? win.value).focus({ preventScroll: true });
});
onBeforeUnmount(() => {
  cancelAnimationFrame(frame);
  if (opener?.isConnected) opener.focus({ preventScroll: true });
});

function begin(kind: "move" | "size", event: PointerEvent) {
  if (!win.value || event.button !== 0 || (event.target as Element).closest("button, input, select, textarea, a")) return;
  const box = win.value.getBoundingClientRect();
  gesture = { kind, startX: event.clientX, startY: event.clientY, x, y, width: box.width, height: box.height };
  (event.currentTarget as Element).setPointerCapture(event.pointerId);
  event.preventDefault();
}
function move(event: PointerEvent) {
  if (!gesture) return;
  pending = event;
  frame ||= requestAnimationFrame(apply);
}
function apply() {
  frame = 0;
  const e = pending;
  if (!win.value || !gesture || !e) return;
  const dx = e.clientX - gesture.startX;
  const dy = e.clientY - gesture.startY;
  if (gesture.kind === "move") {
    x = Math.max(0, Math.min(gesture.x + dx, innerWidth - 60));
    y = Math.max(0, Math.min(gesture.y + dy, innerHeight - 40));
    place();
  } else {
    win.value.style.width = `${Math.max(props.minWidth, Math.min(gesture.width + dx, innerWidth - x))}px`;
    win.value.style.height = `${Math.max(props.minHeight, Math.min(gesture.height + dy, innerHeight - y))}px`;
  }
}
const end = () => (gesture = null);

function key(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.stopPropagation();
    emit("close");
  } else if (props.modal && event.key === "Tab") {
    const items = [...(win.value?.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled), select, textarea, a[href]") ?? [])];
    if (!items.length) return;
    const at = items.indexOf(document.activeElement as HTMLElement);
    items[(at + (event.shiftKey ? -1 : 1) + items.length) % items.length]?.focus();
    event.preventDefault();
  }
}
</script>

<template>
  <div class="layer" :class="{ modal }">
    <section
      ref="win"
      class="fw"
      role="dialog"
      :aria-modal="modal"
      :aria-label="title"
      tabindex="-1"
      data-keep-keys
      :style="{ width: width + 'px' }"
      @keydown="key"
    >
      <header @pointerdown="begin('move', $event)" @pointermove="move" @pointerup="end" @pointercancel="end">
        <strong>{{ title }}</strong>
        <button class="flat icon" :aria-label="t('common.close')" :title="t('common.close')" @click="emit('close')"><Icon name="close" :size="14" /></button>
      </header>
      <div class="body"><slot /></div>
      <div v-if="resizable" class="grip" aria-hidden="true" @pointerdown="begin('size', $event)" @pointermove="move" @pointerup="end" @pointercancel="end" />
    </section>
  </div>
</template>

<style scoped>
.layer { position: fixed; inset: 0; z-index: 60; pointer-events: none; }
.layer.modal { pointer-events: auto; background: rgb(0 0 0 / 28%); }
.fw {
  position: absolute; left: 0; top: 0; max-width: calc(100vw - 16px); max-height: calc(100vh - 16px); display: flex; flex-direction: column; pointer-events: auto;
  background: var(--surface); border: 1px solid var(--line); border-radius: 8px; box-shadow: 0 8px 30px rgb(0 0 0 / 28%); outline: none; will-change: transform;
}
header { display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 12px; border-bottom: 1px solid var(--line); background: var(--surface-2); border-radius: 8px 8px 0 0; cursor: move; touch-action: none; user-select: none; flex: none; }
header strong { flex: 1; font-size: 13px; }
.body { padding: 10px 14px 14px; font-size: 13px; overflow: auto; min-height: 0; flex: 1; }
.grip { position: absolute; right: 0; bottom: 0; width: 16px; height: 16px; cursor: nwse-resize; touch-action: none; border-radius: 0 0 8px 0;
  background: linear-gradient(135deg, transparent 55%, var(--muted) 55%, var(--muted) 62%, transparent 62%, transparent 74%, var(--muted) 74%, var(--muted) 81%, transparent 81%); opacity: 0.6; }
</style>
