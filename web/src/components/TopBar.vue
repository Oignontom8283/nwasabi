<script setup>
import { computed, inject, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { LOCALES, setLocale } from "../i18n.js";
import { ui } from "../ui.js";
import Icon from "./Icon.vue";
import Logo from "./Logo.vue";

const emit = defineEmits(["pick"]);
const { t, locale } = useI18n();
const emulator = inject("emulator");
const { state } = emulator;

const THEMES = ["auto", "light", "dark"];
const THEME_ICON = { auto: "contrast", light: "sun", dark: "moon" };
const themeLabel = computed(() => t("top.theme", { theme: t(`top.themes.${ui.theme}`) }));
const nextTheme = () => (ui.theme = THEMES[(THEMES.indexOf(ui.theme) + 1) % THEMES.length]);

// Le menu des imports : trois traits à gauche ; il se ferme en cliquant ailleurs ou avec Échap.
const open = ref(false);
const root = ref(null);
const ITEMS = [
  ["app", "top.openApp", true],
  ["data", "top.openData", false],
  ["keys", "top.openKeys", false],
  ["files", "top.openFiles", false],
  ["state", "top.openState", false],
];
function pick(which) {
  open.value = false;
  emit("pick", which);
}
const outside = (e) => open.value && !root.value?.contains(e.target) && (open.value = false);
const escape = (e) => e.key === "Escape" && open.value && ((open.value = false), e.stopPropagation());
onMounted(() => {
  addEventListener("pointerdown", outside);
  addEventListener("keydown", escape, true);
});
onBeforeUnmount(() => {
  removeEventListener("pointerdown", outside);
  removeEventListener("keydown", escape, true);
});
</script>

<template>
  <header class="topbar">
    <div ref="root" class="menu-wrap">
      <button class="burger" :aria-expanded="open" aria-haspopup="menu" :aria-label="t('top.menu')" :title="t('top.menu')" @click="open = !open">
        <svg width="22" height="22" viewBox="0 0 22 22" aria-hidden="true"><path d="M3 5h16M3 11h16M3 17h16" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" /></svg>
      </button>
      <ul v-if="open" class="menu" role="menu">
        <li v-for="[id, key, primary] in ITEMS" :key="id" role="none">
          <button role="menuitem" :class="{ primary }" :title="t(`${key}Hint`)" @click="pick(id)">{{ t(key) }}</button>
        </li>
      </ul>
    </div>

    <a class="brand" href="." :title="t('top.about')"><Logo :size="24" /> nwasabi</a>

    <select :value="state.model" :title="t('top.modelHint')" :aria-label="t('top.model')" @change="(e) => emulator.changeModel(e.target.value)">
      <option value="n0110">N0110</option>
      <option value="n0115">N0115</option>
      <option value="n0120">N0120</option>
    </select>
    <select :value="state.firmware" :title="t('top.firmwareHint')" :aria-label="t('top.firmware')" @change="(e) => emulator.changeModel(state.model, e.target.value)">
      <option value="24">Epsilon 23–24</option>
      <option value="26.4.0">Epsilon 26.4.0</option>
    </select>
    <button :disabled="!state.loaded" :title="state.running ? t('top.pauseHint') : t('top.resumeHint')" @click="state.running ? emulator.stop() : emulator.start()">
      <Icon :name="state.running ? 'pause' : 'play'" :size="14" /> {{ state.running ? t("top.pause") : t("top.resume") }}
    </button>
    <button :disabled="!state.loaded" :title="t('top.restartHint')" @click="emulator.restart()"><Icon name="refresh" :size="14" /> {{ t("top.restart") }}</button>

    <span class="name" :class="{ none: !state.appName }" :title="state.appName">{{ state.appName || t("top.noApp") }}</span>

    <div class="right">
      <select :value="locale" :aria-label="t('top.language')" :title="t('top.language')" @change="(e) => setLocale(e.target.value)">
        <option v-for="[code, name] in LOCALES" :key="code" :value="code">{{ name }}</option>
      </select>
      <button class="flat icon" :title="themeLabel" :aria-label="themeLabel" @click="nextTheme"><Icon :name="THEME_ICON[ui.theme]" /></button>
    </div>
  </header>
</template>

<style scoped>
.topbar { height: var(--topbar-h); display: flex; align-items: center; gap: 8px; padding: 0 12px; background: var(--surface); border-bottom: 1px solid var(--line); position: relative; z-index: 20; }
.menu-wrap { position: relative; }
.burger { border: none; background: transparent; padding: 4px; border-radius: 6px; color: var(--text); }
.burger:hover:not(:disabled) { background: var(--accent-soft); }
.menu { position: absolute; top: calc(100% + 6px); left: 0; list-style: none; margin: 0; padding: 6px; min-width: 250px; display: grid; gap: 4px; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; box-shadow: var(--shadow); }
.menu button { width: 100%; justify-content: flex-start; padding: 6px 10px; }
.brand { display: inline-flex; align-items: center; gap: 6px; font-weight: 700; font-size: 16px; color: var(--text); text-decoration: none; margin-right: 6px; }
.name { position: absolute; left: 50%; transform: translateX(-50%); max-width: 28%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; pointer-events: none; }
.name.none { font-weight: 400; color: var(--muted); }
.right { margin-left: auto; display: flex; align-items: center; gap: 6px; }
@media (max-width: 900px) { .name { display: none; } .brand span { display: none; } }
</style>
