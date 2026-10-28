<script setup lang="ts">
// Une variable (ou un champ, un élément) du programme, avec son type et sa valeur ; les structures et les tableaux se déplient.
// Double-clic sur une valeur de variable en mémoire : la modifier (en pause).
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import type { VarValue } from "../../sourceStore.js";
import AddressLink from "../AddressLink.vue";

const props = defineProps<{ item: VarValue; path: string; expr: string; open: Set<string>; derefs: Record<string, VarValue>; editable: boolean; depth?: number }>();
const emit = defineEmits<{ toggle: [path: string, item: VarValue, expr: string]; assign: [item: VarValue, text: string, expr: string]; follow: [address: number] }>();
const { t } = useI18n();

const depth = props.depth ?? 0;
const editing = ref<string | null>(null);
/** Les éléments à montrer en dessous : les champs ou éléments, ou ce que pointe un pointeur (lu à la demande). */
const kids = (): VarValue[] => {
  if (props.item.children?.length) return props.item.children;
  const pointed = props.derefs[props.path];
  if (!pointed) return [];
  return pointed.children?.length ? pointed.children : [{ ...pointed, name: "*" }];
};
const isPointer = () => props.item.pointer !== undefined && !(props.item.children?.length);
const expandable = () => (props.item.children?.length ?? 0) > 0 || isPointer();
/** L'expression de l'élément `child` : champ, élément de tableau, ou objet pointé. */
const childExpr = (child: VarValue, i: number) => {
  const base = isPointer() ? `(*${props.expr})` : props.expr;
  if (child.name === "*") return base;
  return child.name?.startsWith("[") ? `${base}${child.name}` : `${base}.${child.name ?? i}`;
};
const scalar = () => (props.item.children?.length ?? 0) === 0 && !isPointer() && props.item.address !== undefined && !props.item.value.startsWith("<");

function startEdit() {
  if (!props.editable || !scalar()) return;
  editing.value = props.item.value.replace(/ '.*$/, "").replace(/^0x[0-9a-f]+ ".*$/, props.item.value.split(" ")[0] ?? "");
}
function commit() {
  const text = editing.value;
  editing.value = null;
  if (text !== null && text.trim()) emit("assign", props.item, text.trim(), props.expr);
}
</script>

<template>
  <div class="var" :style="{ '--depth': depth }">
    <div class="row">
      <button v-if="expandable()" class="twist" :aria-expanded="open.has(path)" :aria-label="open.has(path) ? t('source.collapse') : t('source.expand')" @click="emit('toggle', path, item, expr)">{{ open.has(path) ? "▾" : "▸" }}</button>
      <span v-else class="twist blank" />
      <span class="name">{{ item.name }}</span>
      <span class="type muted">{{ item.type }}</span>
      <span class="value mono" :class="{ muted: item.value.startsWith('<') }">
        <input v-if="editing !== null" v-model="editing" class="edit mono" data-keep-keys autofocus @keydown.enter.prevent="commit" @keydown.esc.prevent="editing = null" @blur="commit" />
        <span v-else :title="editable && scalar() ? t('source.editHint') : ''" @dblclick="startEdit">{{ (item.children?.length ?? 0) > 0 && open.has(path) ? "" : item.value }}</span>
      </span>
      <AddressLink v-if="item.address !== undefined && depth === 0" class="at" :address="item.address" view="memory" />
    </div>
    <div v-if="expandable() && open.has(path)" class="children">
      <VarTree v-for="(c, i) in kids()" :key="i" :item="c" :path="`${path}/${c.name ?? i}`" :expr="childExpr(c, i)" :open="open" :derefs="derefs" :editable="editable" :depth="depth + 1" @toggle="(p, it, ex) => emit('toggle', p, it, ex)" @assign="(it, tx, ex) => emit('assign', it, tx, ex)" @follow="(a) => emit('follow', a)" />
    </div>
  </div>
</template>

<style scoped>
.var { font-size: 12.5px; }
.row { display: flex; align-items: baseline; gap: 6px; padding: 1px 0 1px calc(var(--depth) * 14px); min-width: 0; }
.twist { width: 14px; flex: none; border: none; background: none; color: var(--muted); padding: 0; cursor: pointer; font-size: 11px; line-height: 1; }
.twist.blank { cursor: default; }
.name { font-weight: 600; flex: none; }
.type { font-size: 11px; flex: none; }
.value { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 0 1 auto; }
.edit { width: 12em; height: 20px; padding: 0 4px; }
.at, .ptr { margin-left: auto; flex: none; font-size: 11px; }
</style>
