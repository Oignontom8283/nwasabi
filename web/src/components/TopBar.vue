<script setup lang="ts">
import { useEmu } from "../emulator.js";
import { computed, inject, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { LOCALES, setLocale } from "../i18n.js";
import { reload, server } from "../serverLink.js";
import { ui } from "../ui.js";
import type { PickKind, Theme } from "../ui.js";
import { valueOf } from "../util.js";
import type { Firmware, Model } from "@nwasabi/js";
import Icon from "./Icon.vue";
import Logo from "./Logo.vue";

const emit = defineEmits<{ pick: [which: PickKind]; about: []; info: [] }>();
const hasInfo = __NWASABI_INFO__ !== "";
const { t, locale } = useI18n();
const emulator = useEmu();
const { state } = emulator;

const REPO = "https://github.com/Oignontom8283/nwasabi";
const THEMES: Theme[] = ["auto", "light", "dark"];
const THEME_ICON: Record<Theme, string> = { auto: "monitor", light: "sun", dark: "moon" };
const themeLabel = computed(() => t("top.theme", { theme: t(`top.themes.${ui.theme}`) }));
const nextTheme = () => (ui.theme = THEMES[(THEMES.indexOf(ui.theme) + 1) % THEMES.length] ?? "auto");

// Le menu des imports : trois traits à gauche ; il se ferme en cliquant ailleurs ou avec Échap.
const open = ref(false);
const root = ref<HTMLElement | null>(null);
const ITEMS: Array<[PickKind, string, boolean]> = [
  ["app", "top.openApp", true],
  ["data", "top.openData", false],
  ["keys", "top.openKeys", false],
  ["files", "top.openFiles", false],
  ["state", "top.openState", false],
];
function pick(which: PickKind) {
  open.value = false;
  emit("pick", which);
}
const outside = (e: Event) => open.value && !root.value?.contains(e.target as Node) && (open.value = false);
const escape = (e: KeyboardEvent) => e.key === "Escape" && open.value && ((open.value = false), e.stopPropagation());
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
        <Icon name="burger" :size="22" />
      </button>
      <ul v-if="open" class="menu" role="menu">
        <li v-for="[id, key, primary] in ITEMS" :key="id" role="none">
          <button role="menuitem" :class="{ primary }" :title="t(`${key}Hint`)" @click="pick(id)">{{ t(key) }}</button>
        </li>
      </ul>
    </div>

    <a class="brand" href="." :title="t('top.about')"><Logo :size="34" /> Nwasabi</a>

    <select :value="state.model" :title="t('top.modelHint')" :aria-label="t('top.model')" @change="(e: Event) => emulator.changeModel(valueOf(e) as Model)">
      <option value="n0110">N0110</option>
      <option value="n0115">N0115</option>
      <option value="n0120">N0120</option>
    </select>
    <select :value="state.firmware" :title="t('top.firmwareHint')" :aria-label="t('top.firmware')" @change="(e: Event) => emulator.changeModel(state.model, valueOf(e) as Firmware)">
      <option value="24">Epsilon 23–24</option>
      <option value="26.4.0">Epsilon 26.4.0</option>
    </select>
    <button :disabled="!state.loaded" :title="state.running ? t('top.pauseHint') : t('top.resumeHint')" @click="state.running ? emulator.stop() : emulator.start()">
      <Icon :name="state.running ? 'pause' : 'play'" :size="14" /> {{ state.running ? t("top.pause") : t("top.resume") }}
    </button>
    <button :disabled="!state.loaded" :title="t('top.restartHint')" @click="emulator.restart()"><Icon name="refresh" :size="14" /> {{ t("top.restart") }}</button>
    <button v-if="server.stale.length" class="reload" :title="t('top.reloadHint')" @click="reload()"><Icon name="refresh" :size="14" /> {{ t("top.reload") }}</button>
    <button v-if="hasInfo" class="icon" :title="t('top.info')" :aria-label="t('top.info')" @click="emit('info')"><Icon name="info" :size="16" /></button>

    <div class="center" :class="{ idle: !state.appName }">
      <span class="name" :class="{ none: !state.appName }" :title="state.appName">{{ state.appName || t("top.noApp") }}</span>
      <button v-if="!state.loaded && !state.needsData" class="load primary" :title="t('top.openAppHint')" @click="pick('app')"><Icon name="upload" :size="14" /> {{ t("top.load") }}</button>
    </div>

    <div class="right">
      <select :value="locale" :aria-label="t('top.language')" :title="t('top.language')" @change="(e: Event) => setLocale(valueOf(e))">
        <option v-for="[code, name] in LOCALES" :key="code" :value="code">{{ name }}</option>
      </select>
      <button class="flat icon" :class="{ on: ui.large }" :title="t('top.large')" :aria-label="t('top.large')" :aria-pressed="ui.large" @click="ui.large = !ui.large"><Icon name="expand" :size="18" /></button>
      <button class="flat icon" :title="themeLabel" :aria-label="themeLabel" @click="nextTheme"><Icon :name="THEME_ICON[ui.theme]" :size="18" /></button>
      <button class="flat icon" :title="t('top.aboutWindow')" :aria-label="t('top.aboutWindow')" @click="emit('about')"><Icon name="license" :size="18" /></button>
      <a class="flat icon link" :href="REPO" target="_blank" rel="noopener noreferrer" :title="t('top.repo')" :aria-label="t('top.repo')"><Icon name="github" :size="18" /></a>
    </div>
  </header>
</template>

<style scoped>
.topbar { height: var(--topbar-h); display: flex; align-items: center; gap: 8px; padding: 0 12px; background: var(--surface); border-bottom: 1px solid var(--line); position: relative; z-index: 20; }
.menu-wrap { position: relative; }
.burger { border: none; background: transparent; padding: 4px; border-radius: 6px; color: var(--text); }
.menu { position: absolute; top: calc(100% + 6px); left: 0; list-style: none; margin: 0; padding: 6px; min-width: 260px; display: grid; gap: 3px; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; box-shadow: var(--shadow); }
.menu button { width: 100%; justify-content: flex-start; padding: 6px 10px; }
.brand { display: inline-flex; align-items: center; gap: 8px; font-weight: 700; font-size: 20px; letter-spacing: 0.01em; color: var(--text); text-decoration: none; margin-right: 18px; }
/* Le nom reste exactement au centre de la barre ; le bouton de chargement se pose à sa droite sans le décaler. */
.center { position: absolute; left: 50%; transform: translateX(-50%); max-width: 28%; }
.name { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted); pointer-events: none; }
.load { position: absolute; left: 100%; top: 50%; transform: translateY(-50%); margin-left: 12px; }
.right { margin-left: auto; display: flex; align-items: center; gap: 4px; }
.right select { margin-right: 4px; }
a.link { display: inline-flex; align-items: center; border-radius: 6px; padding: 3px 6px; color: var(--text); border: 1px solid transparent; }
a.link:hover { background-image: linear-gradient(var(--hover), var(--hover)); }
/* Sans application, le texte « Aucune application » cède la place au seul bouton quand la barre est étroite. */
@media (max-width: 1560px) { .center.idle { transform: none; } .center.idle .name { display: none; } .center.idle .load { position: static; transform: none; margin: 0 0 0 12px; } }
@media (max-width: 1330px) { .center { display: none; } }
@media (max-width: 700px) { .brand span { display: none; } }
</style>
